use std::io;

use anyhow::bail;
use wafer_protocol::{ALWAYS_ENCODABLE_LEN, Codec, Fault, Reader, Writer, stream_codec};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::net::tcp::{OwnedReadHalf, OwnedWriteHalf};
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

use tokio::sync::mpsc;
use tokio::time::{Instant, Sleep};

use super::chat::{Arg, Component};
use super::encryption::{Decryptor, Encryptor};
use crate::util::java_throwable::JavaThrowable;

const READ_TIMEOUT: Duration = Duration::from_secs(30);

pub struct Packet {
    pub id: i32,
    pub data: Vec<u8>,
}

impl Packet {
    pub fn reader(&self) -> Reader<'_> {
        Reader::new(&self.data)
    }
}

pub struct PacketReader {
    half: OwnedReadHalf,
    pub codec: Codec,
    buf: Vec<u8>,
    decryptor: Option<Decryptor>,
    timeout: Pin<Box<Sleep>>,
}

impl PacketReader {
    pub async fn recv(&mut self) -> anyhow::Result<Packet> {
        loop {
            if let Some((bytes, used)) = self.codec.decode_frame(&self.buf)? {
                self.buf.drain(..used);
                let mut r = Reader::new(&bytes);
                let id = r.varint()?;
                let data = bytes[bytes.len() - r.remaining()..].to_vec();
                return Ok(Packet { id, data });
            }
            self.read().await?;
        }
    }

    pub async fn read(&mut self) -> io::Result<&[u8]> {
        let start = self.buf.len();
        self.buf.reserve(8192);
        let n = tokio::select! {
            n = self.half.read_buf(&mut self.buf) => n?,
            _ = &mut self.timeout => return Err(io::Error::from(io::ErrorKind::TimedOut)),
        };
        if n == 0 {
            return Err(io::Error::from(io::ErrorKind::UnexpectedEof));
        }
        self.timeout.as_mut().reset(Instant::now() + READ_TIMEOUT);
        if let Some(decryptor) = &mut self.decryptor {
            decryptor.apply(&mut self.buf[start..]);
        }
        Ok(&self.buf[start..])
    }

    pub fn enable_encryption(&mut self, secret: &[u8; 16]) {
        let mut decryptor = Decryptor::new(secret);
        decryptor.apply(&mut self.buf);
        self.decryptor = Some(decryptor);
    }

    pub async fn expect(&mut self, id: i32) -> anyhow::Result<Packet> {
        let packet = self.recv().await?;
        if packet.id != id {
            bail!("expected packet 0x{id:02x}, got 0x{:02x}", packet.id);
        }
        Ok(packet)
    }
}

pub struct PacketWriter {
    half: OwnedWriteHalf,
    pub codec: Codec,
    out: Vec<u8>,
    encryptor: Option<Encryptor>,
}

impl PacketWriter {
    pub fn queue(&mut self, id: i32, body: impl FnOnce(&mut Writer)) {
        let mut w = Writer::new();
        w.varint(id);
        body(&mut w);
        self.out.extend(self.codec.encode(w.as_slice()));
    }

    pub async fn flush(&mut self) -> io::Result<()> {
        if !self.out.is_empty() {
            if let Some(encryptor) = &mut self.encryptor {
                encryptor.apply(&mut self.out);
            }
            self.half.write_all(&self.out).await?;
            self.out.clear();
        }
        Ok(())
    }

    pub async fn send(&mut self, id: i32, body: impl FnOnce(&mut Writer)) -> io::Result<()> {
        self.queue(id, body);
        self.flush().await
    }

    pub async fn send_raw_and_close(&mut self, bytes: &[u8]) -> io::Result<()> {
        self.half.write_all(bytes).await?;
        self.half.shutdown().await
    }

    pub fn enable_encryption(&mut self, secret: &[u8; 16]) {
        debug_assert!(self.out.is_empty());
        self.encryptor = Some(Encryptor::new(secret));
    }
}

pub fn split(stream: TcpStream) -> io::Result<(PacketReader, PacketWriter)> {
    stream.set_nodelay(true)?;
    let (r, w) = stream.into_split();
    Ok((
        PacketReader { half: r, codec: Codec::default(), buf: Vec::new(), decryptor: None, timeout: Box::pin(tokio::time::sleep(READ_TIMEOUT)) },
        PacketWriter { half: w, codec: Codec::default(), out: Vec::new(), encryptor: None },
    ))
}

#[derive(Clone)]
pub struct Outbound {
    sender: mpsc::UnboundedSender<Vec<u8>>,
    codec: Codec,
    io_thread: Arc<str>,
    marks_fallbacks: bool,
}

impl Outbound {
    pub fn send(&self, id: i32, body: impl FnOnce(&mut Writer)) {
        self.send_raw(encode(id, body));
    }

    pub fn send_raw(&self, packet: Vec<u8>) {
        let _ = self.sender.send(packet);
    }

    fn send_marked(&self, packet: Vec<u8>, fallback_type: Option<i32>) {
        match fallback_type {
            Some(fallback_type) if self.marks_fallbacks => {
                let mut marked = Writer::new();
                marked.bytes(&FALLBACK_MARKER);
                marked.varint(fallback_type);
                marked.bytes(&packet);
                self.send_raw(marked.into_inner());
            }
            _ => self.send_raw(packet),
        }
    }

    pub fn send_with_fallback(&self, id: i32, body: impl FnOnce(&mut Writer), fallback_type: Option<i32>, fallback: impl FnOnce() -> Option<Vec<u8>>) {
        if self.sender.is_closed() {
            return;
        }
        let packet = encode(id, body);
        if let Some((id, fault)) = encoding_fault(&packet) {
            let encoder_exception = encoder_exception(id, fault, NettyPath::Listener);
            error!(thread: &self.io_thread, "Error sending packet {}\n{}", encoder_exception.packet_type, encoder_exception.wrapper.print_stack_trace().trim_end());
            if let Some(fallback) = fallback() {
                warn!(thread: &self.io_thread, "Failed to deliver packet, sending fallback {}\n{}", encoder_exception.packet_type, encoder_exception.skip().print_stack_trace().trim_end());
                self.send_raw(fallback);
            }
            return;
        }
        if packet.len() > ALWAYS_ENCODABLE_LEN
            && let Err(cause) = self.codec.try_encode(&packet)
        {
            if let Some(fallback) = fallback() {
                let name = wafer_data::packet_name("play", "clientbound", id).unwrap_or_default();
                warn!(thread: &self.io_thread, "Failed to deliver packet, sending fallback clientbound/{name}\n{}", cause.join("\n"));
                self.send_raw(fallback);
            }
            return;
        }
        self.send_marked(packet, fallback_type);
    }
}

const EVENT_LOOP_WAKEUP: std::time::Duration = std::time::Duration::from_millis(2);
const FAULTED_PACKET: [u8; 5] = [0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
const FALLBACK_MARKER: [u8; 5] = [0xFF, 0xFF, 0xFF, 0xFF, 0x0E];
const READ_FAILURE_MARKER: [u8; 5] = [0xFF, 0xFF, 0xFF, 0xFF, 0x0D];

fn split_fallback(item: &[u8]) -> (Option<i32>, &[u8]) {
    let Some(rest) = item.strip_prefix(&FALLBACK_MARKER) else { return (None, item) };
    let mut r = Reader::new(rest);
    let Ok(fallback_type) = r.varint() else { return (None, item) };
    (Some(fallback_type), &rest[rest.len() - r.remaining()..])
}

pub fn encode(id: i32, body: impl FnOnce(&mut Writer)) -> Vec<u8> {
    let mut w = Writer::new();
    w.varint(id);
    body(&mut w);
    let Some(fault) = w.faults().first() else {
        return w.into_inner();
    };
    let packet = w.as_slice();
    let id_len = packet.iter().position(|&b| b & 0x80 == 0).map_or(packet.len(), |i| i + 1);
    let mut fault = fault.clone();
    if fault.has_frames() {
        fault.push_frames(stream_codec::MAP_STREAM);
        fault.push_frames(stream_codec::ID_DISPATCH);
    }
    let mut faulted = Vec::with_capacity(FAULTED_PACKET.len() + id_len + 256);
    faulted.extend_from_slice(&FAULTED_PACKET);
    faulted.extend_from_slice(&packet[..id_len]);
    fault.write_to(&mut faulted);
    faulted.extend_from_slice(&crate::commands::execution::tasks::running_command().to_le_bytes());
    faulted
}

fn faulting_command(packet: &[u8]) -> u64 {
    packet.last_chunk::<8>().map_or(0, |serial| u64::from_le_bytes(*serial))
}

async fn wait_for_event_loop(packet: &[u8]) {
    let command = faulting_command(packet);
    let deadline = std::time::Instant::now() + std::time::Duration::from_millis(200);
    while command != 0 && crate::commands::execution::tasks::running_command() == command && std::time::Instant::now() < deadline {
        tokio::time::sleep(std::time::Duration::from_millis(1)).await;
    }
    tokio::time::sleep(EVENT_LOOP_WAKEUP).await;
}

pub(crate) fn encoding_fault(packet: &[u8]) -> Option<(i32, Fault)> {
    let rest = packet.strip_prefix(&FAULTED_PACKET)?;
    let mut r = Reader::new(rest);
    let id = r.varint().ok()?;
    let mut fault = &rest[rest.len() - r.remaining()..];
    Some((id, Fault::read_from(&mut fault)?))
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum NettyPath {
    Plain,
    Listener,
    Bundle,
}

const ABSTRACT_CHANNEL_HANDLER_CONTEXT_WRITE: &str = "io.netty.channel.AbstractChannelHandlerContext.write(AbstractChannelHandlerContext.java:800)";
const MESSAGE_TO_MESSAGE_ENCODER_WRITE: &str = "io.netty.handler.codec.MessageToMessageEncoder.write(MessageToMessageEncoder.java:113)";
const EVENT_LOOP_TAIL: [&str; 7] = [
    "io.netty.util.concurrent.AbstractEventExecutor.runTask(AbstractEventExecutor.java:148)",
    "io.netty.util.concurrent.AbstractEventExecutor.safeExecute(AbstractEventExecutor.java:141)",
    "io.netty.util.concurrent.SingleThreadEventExecutor.runAllTasks(SingleThreadEventExecutor.java:535)",
    "io.netty.channel.SingleThreadIoEventLoop.run(SingleThreadIoEventLoop.java:201)",
    "io.netty.util.concurrent.SingleThreadEventExecutor$5.run(SingleThreadEventExecutor.java:1204)",
    "io.netty.util.internal.ThreadExecutorMap$2.run(ThreadExecutorMap.java:74)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

fn netty_write_path(path: NettyPath) -> Vec<&'static str> {
    let mut frames = vec![
        "io.netty.handler.codec.MessageToByteEncoder.write(MessageToByteEncoder.java:107)",
        ABSTRACT_CHANNEL_HANDLER_CONTEXT_WRITE,
        "io.netty.channel.AbstractChannelHandlerContext.write(AbstractChannelHandlerContext.java:737)",
    ];
    if path == NettyPath::Bundle {
        frames.push("io.netty.handler.codec.MessageToMessageEncoder.writeVoidPromise(MessageToMessageEncoder.java:133)");
        frames.push("io.netty.handler.codec.MessageToMessageEncoder.write(MessageToMessageEncoder.java:118)");
    } else {
        frames.push(MESSAGE_TO_MESSAGE_ENCODER_WRITE);
    }
    frames.extend([
        ABSTRACT_CHANNEL_HANDLER_CONTEXT_WRITE,
        "io.netty.channel.AbstractChannelHandlerContext.write(AbstractChannelHandlerContext.java:737)",
        "io.netty.channel.ChannelOutboundHandlerAdapter.write(ChannelOutboundHandlerAdapter.java:113)",
        "net.minecraft.network.Connection$2.write(Connection.java:487)",
        ABSTRACT_CHANNEL_HANDLER_CONTEXT_WRITE,
    ]);
    if path == NettyPath::Listener {
        frames.extend([
            "io.netty.channel.AbstractChannelHandlerContext.write(AbstractChannelHandlerContext.java:731)",
            "io.netty.channel.DefaultChannelPipeline.write(DefaultChannelPipeline.java:1031)",
            "io.netty.channel.Channel.write(Channel.java:272)",
            "net.minecraft.network.Connection.doSendPacket(Connection.java:323)",
        ]);
    } else {
        frames.extend([
            "io.netty.channel.AbstractChannelHandlerContext.write(AbstractChannelHandlerContext.java:737)",
            "io.netty.channel.DefaultChannelPipeline.write(DefaultChannelPipeline.java:1036)",
            "io.netty.channel.Channel.write(Channel.java:267)",
            "net.minecraft.network.Connection.doSendPacket(Connection.java:329)",
        ]);
    }
    frames.push("net.minecraft.network.Connection.lambda$sendPacket$0(Connection.java:317)");
    frames.extend(EVENT_LOOP_TAIL);
    frames
}

fn throwable_from_fault(fault: &Fault) -> JavaThrowable {
    let throwable = JavaThrowable::new(fault.description.clone(), fault.frames.clone());
    match &fault.cause {
        Some(cause) => throwable.with_cause(throwable_from_fault(cause)),
        None => throwable,
    }
}

pub(super) struct EncoderException {
    pub(super) packet_type: String,
    pub(super) wrapper: JavaThrowable,
    path: NettyPath,
}

impl EncoderException {
    pub(super) fn skip(&self) -> JavaThrowable {
        let mut frames = vec!["net.minecraft.network.PacketEncoder.encode(PacketEncoder.java:43)", "net.minecraft.network.PacketEncoder.encode(PacketEncoder.java:12)"];
        frames.extend(netty_write_path(self.path));
        JavaThrowable::new(format!("net.minecraft.network.SkipPacketEncoderException: {}", self.wrapper.description), frames).with_cause(self.wrapper.clone())
    }
}

pub(super) fn encoder_exception(id: i32, fault: Fault, path: NettyPath) -> EncoderException {
    let packet_type = format!("clientbound/{}", wafer_data::packet_name("play", "clientbound", id).unwrap_or_default());
    let description = format!("io.netty.handler.codec.EncoderException: Failed to encode packet '{packet_type}'");
    let mut wrapper = JavaThrowable::new(description, vec!["net.minecraft.network.codec.IdDispatchCodec.encode(IdDispatchCodec.java:61)"]).with_cause(throwable_from_fault(&fault));
    wrapper.push_frames(&["net.minecraft.network.codec.IdDispatchCodec.encode(IdDispatchCodec.java:14)", "net.minecraft.network.PacketEncoder.encode(PacketEncoder.java:26)", "net.minecraft.network.PacketEncoder.encode(PacketEncoder.java:12)"]);
    wrapper.push_frames(&netty_write_path(path));
    EncoderException { packet_type, wrapper, path }
}

fn bundle_path(bundled: bool) -> NettyPath {
    if bundled { NettyPath::Bundle } else { NettyPath::Plain }
}

fn packet_id(packet: &[u8]) -> Option<i32> {
    Reader::new(packet).varint().ok()
}

fn is_skippable(id: i32) -> bool {
    use wafer_data::packets::play::clientbound::{DISGUISED_CHAT, PLAYER_CHAT, PLAYER_COMBAT_KILL, SYSTEM_CHAT, TAG_QUERY};
    matches!(id, DISGUISED_CHAT | PLAYER_CHAT | PLAYER_COMBAT_KILL | SYSTEM_CHAT | TAG_QUERY)
}

const SOCKET_WRITE: [&str; 9] = [
    "java.base/sun.nio.ch.SocketDispatcher.write0(Native Method)",
    "java.base/sun.nio.ch.SocketDispatcher.write(SocketDispatcher.java:57)",
    "java.base/sun.nio.ch.IOUtil.writeFromNativeBuffer(IOUtil.java:137)",
    "java.base/sun.nio.ch.IOUtil.write(IOUtil.java:81)",
    "java.base/sun.nio.ch.IOUtil.write(IOUtil.java:58)",
    "java.base/sun.nio.ch.SocketChannelImpl.implWrite(SocketChannelImpl.java:562)",
    "java.base/sun.nio.ch.SocketChannelImpl.write(SocketChannelImpl.java:614)",
    "io.netty.channel.socket.nio.NioSocketChannel.doWrite(NioSocketChannel.java:408)",
    "io.netty.channel.AbstractChannel$AbstractUnsafe.flush0(AbstractChannel.java:794)",
];
const SOCKET_GATHERING_WRITE: [&str; 8] = [
    "java.base/sun.nio.ch.SocketDispatcher.writev0(Native Method)",
    "java.base/sun.nio.ch.SocketDispatcher.writev(SocketDispatcher.java:62)",
    "java.base/sun.nio.ch.IOUtil.write(IOUtil.java:227)",
    "java.base/sun.nio.ch.IOUtil.write(IOUtil.java:158)",
    "java.base/sun.nio.ch.SocketChannelImpl.implWrite(SocketChannelImpl.java:593)",
    "java.base/sun.nio.ch.SocketChannelImpl.write(SocketChannelImpl.java:627)",
    "io.netty.channel.socket.nio.NioSocketChannel.doWrite(NioSocketChannel.java:423)",
    "io.netty.channel.AbstractChannel$AbstractUnsafe.flush0(AbstractChannel.java:794)",
];
const WRITE_AND_FLUSH: [&str; 8] = [
    "io.netty.channel.nio.AbstractNioChannel$AbstractNioUnsafe.flush0(AbstractNioChannel.java:404)",
    "io.netty.channel.AbstractChannel$AbstractUnsafe.flush(AbstractChannel.java:758)",
    "io.netty.channel.DefaultChannelPipeline$HeadContext.flush(DefaultChannelPipeline.java:1391)",
    "io.netty.channel.AbstractChannelHandlerContext.flush(AbstractChannelHandlerContext.java:754)",
    "io.netty.channel.ChannelOutboundHandlerAdapter.flush(ChannelOutboundHandlerAdapter.java:125)",
    "io.netty.channel.AbstractChannelHandlerContext.write(AbstractChannelHandlerContext.java:819)",
    "io.netty.channel.AbstractChannelHandlerContext.writeAndFlush(AbstractChannelHandlerContext.java:776)",
    "io.netty.channel.AbstractChannelHandlerContext.writeAndFlush(AbstractChannelHandlerContext.java:859)",
];
const LISTENER_DO_SEND_PACKET: [&str; 3] = [
    "io.netty.channel.DefaultChannelPipeline.writeAndFlush(DefaultChannelPipeline.java:1046)",
    "io.netty.channel.Channel.writeAndFlush(Channel.java:257)",
    "net.minecraft.network.Connection.doSendPacket(Connection.java:323)",
];
const VOID_DO_SEND_PACKET: [&str; 3] = [
    "io.netty.channel.DefaultChannelPipeline.writeAndFlush(DefaultChannelPipeline.java:1041)",
    "io.netty.channel.Channel.writeAndFlush(Channel.java:262)",
    "net.minecraft.network.Connection.doSendPacket(Connection.java:327)",
];
const SEND_PACKET_TASK: &str = "net.minecraft.network.Connection.lambda$sendPacket$0(Connection.java:317)";
const EXCEPTION_CAUGHT_SEND: [&str; 10] = [
    "net.minecraft.network.Connection.sendPacket(Connection.java:315)",
    "net.minecraft.network.Connection.send(Connection.java:297)",
    "net.minecraft.network.Connection.send(Connection.java:291)",
    "net.minecraft.network.Connection.exceptionCaught(Connection.java:142)",
    "io.netty.channel.AbstractChannelHandlerContext.invokeExceptionCaught(AbstractChannelHandlerContext.java:290)",
    "io.netty.channel.AbstractChannelHandlerContext.fireExceptionCaught(AbstractChannelHandlerContext.java:272)",
    "io.netty.channel.DefaultChannelPipeline$HeadContext.exceptionCaught(DefaultChannelPipeline.java:1396)",
    "io.netty.channel.DefaultChannelPipeline.fireExceptionCaught(DefaultChannelPipeline.java:890)",
    "io.netty.channel.nio.AbstractNioByteChannel$NioByteUnsafe.handleReadException(AbstractNioByteChannel.java:132)",
    "io.netty.channel.nio.AbstractNioByteChannel$NioByteUnsafe.read(AbstractNioByteChannel.java:187)",
];
const READ_READY: &str = "io.netty.channel.nio.AbstractNioChannel$AbstractNioUnsafe.handle(AbstractNioChannel.java:445)";
const FORCE_FLUSH: [&str; 2] = [
    "io.netty.channel.nio.AbstractNioChannel$AbstractNioUnsafe.forceFlush(AbstractNioChannel.java:411)",
    "io.netty.channel.nio.AbstractNioChannel$AbstractNioUnsafe.handle(AbstractNioChannel.java:439)",
];
const SELECTED_KEY: [&str; 10] = [
    "io.netty.channel.nio.NioIoHandler$DefaultNioRegistration.handle(NioIoHandler.java:388)",
    "io.netty.channel.nio.NioIoHandler.processSelectedKey(NioIoHandler.java:596)",
    "io.netty.channel.nio.NioIoHandler.processSelectedKeysPlain(NioIoHandler.java:541)",
    "io.netty.channel.nio.NioIoHandler.processSelectedKeys(NioIoHandler.java:514)",
    "io.netty.channel.nio.NioIoHandler.run(NioIoHandler.java:484)",
    "io.netty.channel.SingleThreadIoEventLoop.runIo(SingleThreadIoEventLoop.java:225)",
    "io.netty.channel.SingleThreadIoEventLoop.run(SingleThreadIoEventLoop.java:196)",
    "io.netty.util.concurrent.SingleThreadEventExecutor$5.run(SingleThreadEventExecutor.java:1204)",
    "io.netty.util.internal.ThreadExecutorMap$2.run(ThreadExecutorMap.java:74)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];
const CLOSED_CHANNEL_WRITE: &str = "io.netty.channel.AbstractChannel$AbstractUnsafe.write(Object, ChannelPromise)(Unknown Source)";

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Flushed {
    SendPacket { listener: bool },
    ExceptionCaught,
    Pending { buffers: usize },
}

fn write_exception(error: &io::Error, flushed: Flushed) -> JavaThrowable {
    let description = match error.kind() {
        io::ErrorKind::ConnectionReset => "java.io.IOException: Connection reset by peer".to_string(),
        _ if error.raw_os_error().is_some() => format!("java.io.IOException: {}", crate::util::java_nio::os_message(error).trim_end().trim_end_matches('.')),
        _ => format!("java.io.IOException: {error}"),
    };
    let mut frames = Vec::new();
    match flushed {
        Flushed::Pending { buffers } => {
            frames.extend_from_slice(if buffers > 1 { &SOCKET_GATHERING_WRITE } else { &SOCKET_WRITE });
            frames.extend(FORCE_FLUSH);
            frames.extend(SELECTED_KEY);
        }
        Flushed::SendPacket { listener } => {
            frames.extend(SOCKET_WRITE);
            frames.extend(WRITE_AND_FLUSH);
            if listener {
                frames.extend(LISTENER_DO_SEND_PACKET);
            } else {
                frames.pop();
                frames.extend(VOID_DO_SEND_PACKET);
            }
            frames.push(SEND_PACKET_TASK);
            frames.extend(EVENT_LOOP_TAIL);
        }
        Flushed::ExceptionCaught => {
            frames.extend(SOCKET_WRITE);
            frames.extend(WRITE_AND_FLUSH);
            frames.extend(LISTENER_DO_SEND_PACKET);
            frames.extend(EXCEPTION_CAUGHT_SEND);
            frames.push(READ_READY);
            frames.extend(SELECTED_KEY);
        }
    }
    JavaThrowable::new(description, frames)
}

fn closed_channel_exception(cause: Option<&JavaThrowable>) -> JavaThrowable {
    let exception = JavaThrowable::new("io.netty.channel.StacklessClosedChannelException", vec![CLOSED_CHANNEL_WRITE]);
    match cause {
        Some(cause) => exception.with_cause(cause.clone()),
        None => exception,
    }
}

fn is_read_exception(error: &anyhow::Error) -> bool {
    error.downcast_ref::<io::Error>().is_some_and(|error| !matches!(error.kind(), io::ErrorKind::UnexpectedEof | io::ErrorKind::TimedOut))
}

fn read_exception_description(error: &anyhow::Error) -> String {
    match error.downcast_ref::<io::Error>() {
        Some(error) if error.kind() == io::ErrorKind::ConnectionReset => "java.net.SocketException: Connection reset".into(),
        Some(error) if error.raw_os_error().is_some() => format!("java.io.IOException: {}", crate::util::java_nio::os_message(error).trim_end().trim_end_matches('.')),
        _ => format!("java.io.IOException: {error}"),
    }
}

struct Written {
    end: usize,
    fallback_type: Option<i32>,
    read_failure: bool,
}

async fn write_flushed(half: &mut OwnedWriteHalf, frames: &[u8], written_items: &[Written]) -> Result<(), (io::Error, Flushed, usize)> {
    let mut written = 0;
    let mut blocked = false;
    while written < frames.len() {
        match half.try_write(&frames[written..]) {
            Ok(n) => written += n,
            Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                blocked = true;
                if let Err(error) = half.writable().await {
                    let failing = written_items.iter().position(|item| item.end > written).unwrap_or(written_items.len());
                    return Err((error, Flushed::Pending { buffers: written_items.len() - failing }, failing));
                }
            }
            Err(error) => {
                let failing = written_items.iter().position(|item| item.end > written).unwrap_or(written_items.len());
                let flushed = match written_items.get(failing) {
                    _ if blocked => Flushed::Pending { buffers: written_items.len() - failing },
                    Some(item) if item.read_failure => Flushed::ExceptionCaught,
                    item => Flushed::SendPacket { listener: item.is_some_and(|item| item.fallback_type.is_some()) },
                };
                return Err((error, flushed, failing));
            }
        }
    }
    Ok(())
}

fn failed_to_deliver(write_thread: &str, fallback_type: i32, cause: &JavaThrowable) {
    let name = wafer_data::packet_name("play", "clientbound", fallback_type).unwrap_or_default();
    warn!(thread: write_thread, "Failed to deliver packet, sending fallback clientbound/{name}\n{}", cause.print_stack_trace().trim_end());
}

fn fail_queued(queued: Vec<Vec<u8>>, bundled: &mut bool, write_thread: &str, cause: &JavaThrowable) {
    for item in queued {
        if item.starts_with(&READ_FAILURE_MARKER) {
            continue;
        }
        let (fallback_type, packet) = split_fallback(&item);
        if packet_id(packet) == Some(wafer_data::packets::play::clientbound::BUNDLE_DELIMITER) {
            *bundled = !*bundled;
            continue;
        }
        if *bundled {
            continue;
        }
        if let Some((id, encoding_fault)) = encoding_fault(packet) {
            let encoder_exception = encoder_exception(id, encoding_fault, NettyPath::Plain);
            error!(thread: write_thread, "Error sending packet {}\n{}", encoder_exception.packet_type, encoder_exception.wrapper.print_stack_trace().trim_end());
        } else if let Some(fallback_type) = fallback_type {
            failed_to_deliver(write_thread, fallback_type, cause);
        }
    }
}

pub fn disconnection_reason(error: &anyhow::Error) -> Component {
    match error.downcast_ref::<io::Error>().map(io::Error::kind) {
        Some(io::ErrorKind::TimedOut) => Component::translatable("disconnect.timeout"),
        _ => Component::translatable("multiplayer.disconnect.generic"),
    }
}

pub fn spawn_io(
    mut r: PacketReader,
    mut w: PacketWriter,
    io_thread: Arc<str>,
    on_packet: impl Fn(Result<Packet, Component>) + Send + Sync + 'static,
) -> Outbound {
    let (sender, mut rx) = mpsc::unbounded_channel::<Vec<u8>>();
    let codec = w.codec;
    let on_packet = Arc::new(on_packet);
    let on_read = on_packet.clone();
    let write_thread = io_thread.clone();
    let read_failures = sender.clone();
    let reader = tokio::spawn(async move {
        loop {
            match r.recv().await {
                Ok(packet) => on_read(Ok(packet)),
                Err(e) => {
                    if is_read_exception(&e) {
                        let reason = Component::translatable_with("disconnect.genericReason", vec![Arg::String(format!("Internal Exception: {}", read_exception_description(&e)))]);
                        let mut disconnect = READ_FAILURE_MARKER.to_vec();
                        disconnect.extend(encode(wafer_data::packets::play::clientbound::DISCONNECT, |p| reason.write(p)));
                        let _ = read_failures.send(disconnect);
                    }
                    return on_read(Err(disconnection_reason(&e)));
                }
            }
        }
    });
    tokio::spawn(async move {
        let mut bundled = false;
        let mut written_items = Vec::new();
        while let Some(first) = rx.recv().await {
            let mut frames = Vec::new();
            written_items.clear();
            let mut fault = None;
            let mut next = Some(first);
            while let Some(item) = next.take().or_else(|| rx.try_recv().ok()) {
                if let Some(disconnect) = item.strip_prefix(&READ_FAILURE_MARKER) {
                    frames.extend(w.codec.encode(disconnect));
                    written_items.push(Written { end: frames.len(), fallback_type: None, read_failure: true });
                    break;
                }
                let (fallback_type, packet) = split_fallback(&item);
                if let Some((id, encoding_fault)) = encoding_fault(packet) {
                    let encoder_exception = encoder_exception(id, encoding_fault, bundle_path(bundled));
                    wait_for_event_loop(packet).await;
                    error!(thread: &write_thread, "Error sending packet {}\n{}", encoder_exception.packet_type, encoder_exception.wrapper.print_stack_trace().trim_end());
                    if is_skippable(id) {
                        continue;
                    }
                    fault = Some(vec![encoder_exception.wrapper.description]);
                    break;
                }
                if packet_id(packet) == Some(wafer_data::packets::play::clientbound::BUNDLE_DELIMITER) {
                    bundled = !bundled;
                }
                match w.codec.try_encode(packet) {
                    Ok(frame) => {
                        frames.extend(frame);
                        written_items.push(Written { end: frames.len(), fallback_type, read_failure: false });
                    }
                    Err(cause) => {
                        fault = Some(cause);
                        break;
                    }
                }
            }
            if fault.is_some() {
                rx.close();
                let queued = std::iter::from_fn(|| rx.try_recv().ok()).collect();
                fail_queued(queued, &mut bundled, &write_thread, &closed_channel_exception(None));
            }
            let reason = fault.map(|cause| Component::translatable_with("disconnect.genericReason", vec![Arg::String(format!("Internal Exception: {}", cause[0]))]));
            if let Some(reason) = &reason {
                frames.extend(w.codec.encode(&encode(wafer_data::packets::play::clientbound::DISCONNECT, |p| reason.write(p))));
            }
            if let Some(encryptor) = &mut w.encryptor {
                encryptor.apply(&mut frames);
            }
            let written = write_flushed(&mut w.half, &frames, &written_items).await;
            if let Some(reason) = reason {
                rx.close();
                on_packet(Err(reason));
                break;
            }
            if let Err((error, flushed, failing)) = written {
                rx.close();
                let queued: Vec<Vec<u8>> = std::iter::from_fn(|| rx.try_recv().ok()).collect();
                let flushed = match flushed {
                    Flushed::Pending { buffers } => Flushed::Pending { buffers: buffers + queued.len() },
                    flushed => flushed,
                };
                let cause = write_exception(&error, flushed);
                let closed = closed_channel_exception(Some(&cause));
                let pending = matches!(flushed, Flushed::Pending { .. });
                for (index, item) in written_items[failing..].iter().enumerate() {
                    if let Some(fallback_type) = item.fallback_type {
                        failed_to_deliver(&write_thread, fallback_type, if index == 0 || pending { &cause } else { &closed });
                    }
                }
                fail_queued(queued, &mut bundled, &write_thread, if pending { &cause } else { &closed });
                reader.abort();
                on_packet(Err(Component::translatable("multiplayer.disconnect.generic")));
                break;
            }
            if written_items.last().is_some_and(|item| item.read_failure) {
                rx.close();
                let queued = std::iter::from_fn(|| rx.try_recv().ok()).collect();
                fail_queued(queued, &mut bundled, &write_thread, &closed_channel_exception(None));
                break;
            }
        }
        let _ = w.half.shutdown().await;
    });
    Outbound { sender, codec, io_thread, marks_fallbacks: true }
}

impl Outbound {
    pub fn detached() -> Outbound {
        let (sender, _) = mpsc::unbounded_channel();
        Outbound { sender, codec: Codec::default(), io_thread: Arc::from("Netty NIO IO #1"), marks_fallbacks: false }
    }
}

#[cfg(test)]
impl Outbound {
    pub fn capture() -> (Outbound, mpsc::UnboundedReceiver<Vec<u8>>) {
        let (sender, rx) = mpsc::unbounded_channel();
        (Outbound { sender, codec: Codec::default(), io_thread: Arc::from("Netty NIO IO #1"), marks_fallbacks: false }, rx)
    }
}

#[cfg(test)]
mod tests {
    use wafer_data::packets::play::clientbound::{SET_ENTITY_DATA, SYSTEM_CHAT};

    use super::*;

    #[test]
    fn failed_encodes_carry_their_cause() {
        let ok = encode(SYSTEM_CHAT, |w| w.string("hi"));
        assert!(encoding_fault(&ok).is_none());
        let faulted = encode(SET_ENTITY_DATA, |w| w.string(&"a".repeat(40000)));
        let (id, fault) = encoding_fault(&faulted).unwrap();
        assert_eq!(id, SET_ENTITY_DATA);
        let exception = encoder_exception(id, fault, NettyPath::Plain);
        assert_eq!(exception.packet_type, "clientbound/minecraft:set_entity_data");
        assert_eq!(exception.wrapper.description, "io.netty.handler.codec.EncoderException: Failed to encode packet 'clientbound/minecraft:set_entity_data'");
        assert_eq!(exception.wrapper.cause.as_deref().map(|cause| cause.description.as_str()), Some("io.netty.handler.codec.EncoderException: String too big (was 40000 characters, max 32767)"));
        assert!(is_skippable(SYSTEM_CHAT));
        assert!(!is_skippable(SET_ENTITY_DATA));
    }

    #[test]
    fn fallback_markers_survive_the_queue() {
        let packet = encode(SYSTEM_CHAT, |w| w.string("hi"));
        let (out, _rx) = Outbound::capture();
        let out = Outbound { marks_fallbacks: true, ..out };
        let (sender, mut rx) = mpsc::unbounded_channel();
        let out = Outbound { sender, ..out };
        out.send_with_fallback(SYSTEM_CHAT, |w| w.string("hi"), Some(SYSTEM_CHAT), || None);
        out.send_with_fallback(SYSTEM_CHAT, |w| w.string("hi"), None, || None);
        let marked = rx.try_recv().unwrap();
        assert_eq!(split_fallback(&marked), (Some(SYSTEM_CHAT), &packet[..]));
        let plain = rx.try_recv().unwrap();
        assert_eq!(split_fallback(&plain), (None, &packet[..]));
    }

    fn with_first_line(fixture: &str, first_line: &str) -> String {
        let (_, rest) = fixture.split_once('\n').unwrap();
        format!("{first_line}\n{}", rest.trim_end())
    }

    #[test]
    fn a_send_that_hits_an_aborted_connection_fails_like_vanilla() {
        let aborted = io::Error::from_raw_os_error(if cfg!(windows) { 10053 } else { 103 });
        let cause = write_exception(&aborted, Flushed::SendPacket { listener: true });
        let first_line = format!("java.io.IOException: {}", crate::util::java_nio::os_message(&aborted).trim_end().trim_end_matches('.'));
        assert_eq!(cause.print_stack_trace().trim_end(), with_first_line(include_str!("connection_fixtures/aborted_send.txt"), &first_line));
        let closed = closed_channel_exception(Some(&cause)).print_stack_trace();
        let fixture = include_str!("connection_fixtures/closed_after_aborted_send.txt").replacen(
            "Caused by: java.io.IOException: 현재 연결은 사용자의 호스트 시스템의 소프트웨어의 의해 중단되었습니다",
            &format!("Caused by: {first_line}"),
            1,
        );
        assert_eq!(closed.trim_end(), fixture.trim_end());
    }

    #[test]
    fn a_disconnect_after_a_read_reset_that_cannot_be_written_closes_the_channel_like_vanilla() {
        let cause = write_exception(&io::Error::from(io::ErrorKind::ConnectionReset), Flushed::ExceptionCaught);
        assert_eq!(closed_channel_exception(Some(&cause)).print_stack_trace().trim_end(), include_str!("connection_fixtures/closed_after_read_reset.txt").trim_end());
    }

    #[test]
    fn packets_pending_on_a_blocked_socket_fail_with_the_reset_like_vanilla() {
        let cause = write_exception(&io::Error::from(io::ErrorKind::ConnectionReset), Flushed::Pending { buffers: 2 });
        assert_eq!(cause.print_stack_trace().trim_end(), include_str!("connection_fixtures/pending_reset.txt").trim_end());
        let single = write_exception(&io::Error::from(io::ErrorKind::ConnectionReset), Flushed::Pending { buffers: 1 });
        assert_eq!(single.stack_trace[0], "java.base/sun.nio.ch.SocketDispatcher.write0(Native Method)");
        assert_eq!(single.stack_trace[9], "io.netty.channel.nio.AbstractNioChannel$AbstractNioUnsafe.forceFlush(AbstractNioChannel.java:411)");
    }

    #[test]
    fn a_send_without_a_listener_flushes_through_the_void_promise() {
        let cause = write_exception(&io::Error::from(io::ErrorKind::ConnectionReset), Flushed::SendPacket { listener: false });
        let frames = &cause.stack_trace[SOCKET_WRITE.len() + WRITE_AND_FLUSH.len() - 1..];
        assert_eq!(
            &frames[..4],
            [
                "io.netty.channel.DefaultChannelPipeline.writeAndFlush(DefaultChannelPipeline.java:1041)",
                "io.netty.channel.Channel.writeAndFlush(Channel.java:262)",
                "net.minecraft.network.Connection.doSendPacket(Connection.java:327)",
                SEND_PACKET_TASK,
            ]
        );
    }

    #[tokio::test]
    async fn a_reset_client_fails_the_queued_system_chat_on_the_write() {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        client.set_linger(Some(Duration::ZERO)).unwrap();
        drop(client);
        let (_, mut writer) = split(server).unwrap();
        tokio::time::sleep(Duration::from_millis(50)).await;
        let frames = vec![0; 64];
        let written = [Written { end: 32, fallback_type: None, read_failure: false }, Written { end: 64, fallback_type: Some(SYSTEM_CHAT), read_failure: false }];
        let (_, flushed, failing) = write_flushed(&mut writer.half, &frames, &written).await.unwrap_err();
        assert_eq!((flushed, failing), (Flushed::SendPacket { listener: false }, 0));
    }

    fn reason_text(error: io::Error) -> String {
        disconnection_reason(&anyhow::Error::from(error)).get_string()
    }

    #[test]
    fn read_timeouts_and_closes_log_like_vanilla() {
        assert_eq!(reason_text(io::Error::from(io::ErrorKind::TimedOut)), "Timed out");
        assert_eq!(reason_text(io::Error::from(io::ErrorKind::UnexpectedEof)), "Disconnected");
        assert_eq!(reason_text(io::Error::from(io::ErrorKind::ConnectionReset)), "Disconnected");
        assert_eq!(disconnection_reason(&anyhow::anyhow!("bad packet")).get_string(), "Disconnected");
    }

    #[test]
    fn aborted_reads_are_logged_as_a_plain_disconnect_like_vanilla() {
        assert_eq!(reason_text(io::Error::from(io::ErrorKind::ConnectionAborted)), "Disconnected");
        assert_eq!(reason_text(io::Error::from_raw_os_error(if cfg!(windows) { 10053 } else { 103 })), "Disconnected");
    }

    async fn closed_client(reset: bool) -> io::Error {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let client = tokio::net::TcpStream::connect(listener.local_addr().unwrap()).await.unwrap();
        let (server, _) = listener.accept().await.unwrap();
        if reset {
            client.set_linger(Some(Duration::ZERO)).unwrap();
        }
        drop(client);
        let (mut reader, _writer) = split(server).unwrap();
        reader.recv().await.err().unwrap().downcast::<io::Error>().unwrap()
    }

    #[tokio::test]
    async fn a_reset_connection_is_logged_as_a_plain_disconnect_like_vanilla() {
        let error = closed_client(true).await;
        assert_eq!(error.kind(), io::ErrorKind::ConnectionReset);
        assert_eq!(reason_text(error), "Disconnected");
    }

    #[tokio::test]
    async fn a_closed_connection_is_logged_as_a_plain_disconnect() {
        let error = closed_client(false).await;
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
        assert_eq!(reason_text(error), "Disconnected");
    }
}
