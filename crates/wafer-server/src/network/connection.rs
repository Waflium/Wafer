use std::io;

use anyhow::bail;
use wafer_protocol::{ALWAYS_ENCODABLE_LEN, Codec, Reader, Writer};
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
        if let Some((id, chain)) = encoding_fault(&packet) {
            let (packet_type, wrapper, causes) = encoder_exception(id, chain);
            error!(thread: &self.io_thread, "Error sending packet {packet_type}\n{wrapper}{causes}");
            if let Some(fallback) = fallback() {
                warn!(thread: &self.io_thread, "Failed to deliver packet, sending fallback {packet_type}\nnet.minecraft.network.SkipPacketEncoderException: {wrapper}\nCaused by: {wrapper}{causes}");
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

const FAULTED_PACKET: [u8; 5] = [0xFF, 0xFF, 0xFF, 0xFF, 0x0F];
const FALLBACK_MARKER: [u8; 5] = [0xFF, 0xFF, 0xFF, 0xFF, 0x0E];

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
    let mut faulted = Vec::with_capacity(FAULTED_PACKET.len() + id_len + fault.len());
    faulted.extend_from_slice(&FAULTED_PACKET);
    faulted.extend_from_slice(&packet[..id_len]);
    faulted.extend_from_slice(fault.as_bytes());
    faulted
}

fn encoding_fault(packet: &[u8]) -> Option<(i32, &str)> {
    let rest = packet.strip_prefix(&FAULTED_PACKET)?;
    let mut r = Reader::new(rest);
    let id = r.varint().ok()?;
    Some((id, std::str::from_utf8(&rest[rest.len() - r.remaining()..]).unwrap_or_default()))
}

fn encoder_exception(id: i32, chain: &str) -> (String, String, String) {
    let packet_type = format!("clientbound/{}", wafer_data::packet_name("play", "clientbound", id).unwrap_or_default());
    let wrapper = format!("io.netty.handler.codec.EncoderException: Failed to encode packet '{packet_type}'");
    let causes = chain.lines().map(|line| if line.starts_with('\t') { format!("\n{line}") } else { format!("\nCaused by: {line}") }).collect();
    (packet_type, wrapper, causes)
}

fn is_skippable(id: i32) -> bool {
    use wafer_data::packets::play::clientbound::{DISGUISED_CHAT, PLAYER_CHAT, PLAYER_COMBAT_KILL, SYSTEM_CHAT, TAG_QUERY};
    matches!(id, DISGUISED_CHAT | PLAYER_CHAT | PLAYER_COMBAT_KILL | SYSTEM_CHAT | TAG_QUERY)
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
    tokio::spawn(async move {
        loop {
            match r.recv().await {
                Ok(packet) => on_read(Ok(packet)),
                Err(e) => return on_read(Err(disconnection_reason(&e))),
            }
        }
    });
    tokio::spawn(async move {
        while let Some(first) = rx.recv().await {
            let mut frames = Vec::new();
            let mut fault = None;
            let mut next = Some(first);
            while let Some(item) = next.take().or_else(|| rx.try_recv().ok()) {
                let packet = split_fallback(&item).1;
                if let Some((id, chain)) = encoding_fault(packet) {
                    let (packet_type, wrapper, causes) = encoder_exception(id, chain);
                    error!(thread: &write_thread, "Error sending packet {packet_type}\n{wrapper}{causes}");
                    if is_skippable(id) {
                        continue;
                    }
                    fault = Some(vec![wrapper]);
                    break;
                }
                match w.codec.try_encode(packet) {
                    Ok(frame) => frames.extend(frame),
                    Err(cause) => {
                        fault = Some(cause);
                        break;
                    }
                }
            }
            if fault.is_some() {
                rx.close();
                while let Ok(item) = rx.try_recv() {
                    if let (Some(fallback_type), _) = split_fallback(&item) {
                        let name = wafer_data::packet_name("play", "clientbound", fallback_type).unwrap_or_default();
                        warn!(thread: &write_thread, "Failed to deliver packet, sending fallback clientbound/{name}\nio.netty.channel.StacklessClosedChannelException\n\tat io.netty.channel.AbstractChannel$AbstractUnsafe.write(Object, ChannelPromise)(Unknown Source)");
                    }
                }
            }
            let reason = fault.map(|cause| Component::translatable_with("disconnect.genericReason", vec![Arg::String(format!("Internal Exception: {}", cause[0]))]));
            if let Some(reason) = &reason {
                frames.extend(w.codec.encode(&encode(wafer_data::packets::play::clientbound::DISCONNECT, |p| reason.write(p))));
            }
            if let Some(encryptor) = &mut w.encryptor {
                encryptor.apply(&mut frames);
            }
            let written = w.half.write_all(&frames).await.is_ok();
            if let Some(reason) = reason {
                rx.close();
                on_packet(Err(reason));
                break;
            }
            if !written {
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
        let (id, chain) = encoding_fault(&faulted).unwrap();
        assert_eq!(id, SET_ENTITY_DATA);
        let (packet_type, wrapper, causes) = encoder_exception(id, chain);
        assert_eq!(packet_type, "clientbound/minecraft:set_entity_data");
        assert_eq!(wrapper, "io.netty.handler.codec.EncoderException: Failed to encode packet 'clientbound/minecraft:set_entity_data'");
        assert_eq!(causes, "\nCaused by: io.netty.handler.codec.EncoderException: String too big (was 40000 characters, max 32767)");
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
}
