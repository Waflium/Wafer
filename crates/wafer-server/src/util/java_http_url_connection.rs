use std::io;
use std::pin::Pin;
use std::sync::{Arc, Mutex, OnceLock};
use std::task::{Context, Poll};
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, ReadBuf};
use tokio::net::TcpStream;
use tokio::time::Sleep;
use tokio_rustls::TlsConnector;
use tokio_rustls::rustls::pki_types::{CertificateDer, ServerName};
use tokio_rustls::rustls::{self, ClientConfig, RootCertStore};

use crate::util::java_throwable::JavaThrowable;
use crate::util::java_url::JavaUrl;

pub const CONNECT_TIMEOUT: Duration = Duration::from_millis(5000);
pub const READ_TIMEOUT: Duration = Duration::from_millis(5000);

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Method {
    Get,
    Post,
}

impl Method {
    fn name(self) -> &'static str {
        match self {
            Method::Get => "GET",
            Method::Post => "POST",
        }
    }
}

#[derive(Debug)]
pub struct Response {
    pub status: u16,
    pub content_type: Option<String>,
    pub body: Vec<u8>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Phase {
    Header,
    Body { status: u16 },
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Handshake {
    ReadTimedOut,
    ResetOnRead,
    ResetOnWrite,
    Eof,
    Unrecognized,
    Alert(String),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Failure {
    UnknownHost(String),
    ConnectionRefused,
    ConnectTimedOut,
    Handshake(Handshake),
    ReadTimedOut(Phase),
    ConnectionReset(Phase),
    ConnectionAborted(Phase, String),
    UnexpectedEof,
    PrematureEof { chunked: bool, status: u16 },
    Other(String),
}

const READ_SERVICE_RESPONSE_130: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:130)";
const READ_SERVICE_RESPONSE_141: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:141)";
const READ_SERVICE_RESPONSE_147: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.readServiceResponse(MinecraftClient.java:147)";
const BUILD_HTTP_EXCEPTION_164: &str = "com.mojang.authlib.minecraft.client.MinecraftClient.buildHttpException(MinecraftClient.java:164)";

const HTTPS_CONNECT_FRAMES: [&str; 16] = [
    "java.base/java.net.SocksSocketImpl.connect(SocksSocketImpl.java:284)",
    "java.base/java.net.Socket.connect(Socket.java:659)",
    "java.base/sun.security.ssl.SSLSocketImpl.connect(SSLSocketImpl.java:304)",
    "java.base/sun.net.NetworkClient.doConnect(NetworkClient.java:161)",
    "java.base/sun.net.www.http.HttpClient.openServer(HttpClient.java:516)",
    "java.base/sun.net.www.http.HttpClient.openServer(HttpClient.java:604)",
    "java.base/sun.net.www.protocol.https.HttpsClient.<init>(HttpsClient.java:206)",
    "java.base/sun.net.www.protocol.https.HttpsClient.New(HttpsClient.java:288)",
    "java.base/sun.net.www.protocol.https.AbstractDelegateHttpsURLConnection.getNewHttpClient(AbstractDelegateHttpsURLConnection.java:196)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.plainConnect0(HttpURLConnection.java:1008)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.plainConnect(HttpURLConnection.java:899)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.connect(HttpURLConnection.java:872)",
    "java.base/sun.net.www.protocol.https.AbstractDelegateHttpsURLConnection.connect(AbstractDelegateHttpsURLConnection.java:181)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream0(HttpURLConnection.java:1377)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream(HttpURLConnection.java:1302)",
    "java.base/java.net.HttpURLConnection.getResponseCode(HttpURLConnection.java:493)",
];

const HTTP_CONNECT_FRAMES: [&str; 14] = [
    "java.base/java.net.Socket.connect(Socket.java:659)",
    "java.base/sun.net.NetworkClient.doConnect(NetworkClient.java:161)",
    "java.base/sun.net.www.http.HttpClient.openServer(HttpClient.java:516)",
    "java.base/sun.net.www.http.HttpClient.openServer(HttpClient.java:604)",
    "java.base/sun.net.www.http.HttpClient.<init>(HttpClient.java:276)",
    "java.base/sun.net.www.http.HttpClient.New(HttpClient.java:380)",
    "java.base/sun.net.www.http.HttpClient.New(HttpClient.java:393)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getNewHttpClient(HttpURLConnection.java:1030)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.plainConnect0(HttpURLConnection.java:1008)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.plainConnect(HttpURLConnection.java:899)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.connect(HttpURLConnection.java:872)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream0(HttpURLConnection.java:1377)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream(HttpURLConnection.java:1302)",
    "java.base/java.net.HttpURLConnection.getResponseCode(HttpURLConnection.java:493)",
];

const HTTPS_GET_RESPONSE_CODE: &str = "java.base/sun.net.www.protocol.https.HttpsURLConnectionImpl.getResponseCode(HttpsURLConnectionImpl.java:307)";

const SOCKET_READ_TIMED_OUT: [&str; 2] = ["java.base/sun.nio.ch.NioSocketImpl.timedRead(NioSocketImpl.java:277)", "java.base/sun.nio.ch.NioSocketImpl.implRead(NioSocketImpl.java:302)"];
const SOCKET_READ_RESET: [&str; 1] = ["java.base/sun.nio.ch.NioSocketImpl.implRead(NioSocketImpl.java:316)"];
const SOCKET_READ: [&str; 4] = [
    "java.base/sun.nio.ch.NioSocketImpl.read(NioSocketImpl.java:354)",
    "java.base/sun.nio.ch.NioSocketImpl$1.read(NioSocketImpl.java:798)",
    "java.base/java.net.Socket$SocketInputStream.implRead(Socket.java:974)",
    "java.base/java.net.Socket$SocketInputStream.read(Socket.java:964)",
];
const SOCKET_WRITE: [&str; 8] = [
    "java.base/sun.nio.ch.SocketDispatcher.write0(Native Method)",
    "java.base/sun.nio.ch.SocketDispatcher.write(SocketDispatcher.java:57)",
    "java.base/sun.nio.ch.NioSocketImpl.tryWrite(NioSocketImpl.java:402)",
    "java.base/sun.nio.ch.NioSocketImpl.implWrite(NioSocketImpl.java:418)",
    "java.base/sun.nio.ch.NioSocketImpl.write(NioSocketImpl.java:448)",
    "java.base/sun.nio.ch.NioSocketImpl$2.write(NioSocketImpl.java:821)",
    "java.base/java.net.Socket$SocketOutputStream.implWrite(Socket.java:1086)",
    "java.base/java.net.Socket$SocketOutputStream.write(Socket.java:1076)",
];

const TLS_RECORD_READ: [&str; 2] = ["java.base/sun.security.ssl.SSLSocketInputRecord.read(SSLSocketInputRecord.java:489)", "java.base/sun.security.ssl.SSLSocketInputRecord.readHeader(SSLSocketInputRecord.java:483)"];
const TLS_HANDSHAKE_DECODE: [&str; 6] = [
    "java.base/sun.security.ssl.SSLSocketInputRecord.decode(SSLSocketInputRecord.java:160)",
    "java.base/sun.security.ssl.SSLTransport.decode(SSLTransport.java:111)",
    "java.base/sun.security.ssl.SSLSocketImpl.decode(SSLSocketImpl.java:1506)",
    "java.base/sun.security.ssl.SSLSocketImpl.readHandshakeRecord(SSLSocketImpl.java:1421)",
    "java.base/sun.security.ssl.SSLSocketImpl.startHandshake(SSLSocketImpl.java:455)",
    "java.base/sun.security.ssl.SSLSocketImpl.startHandshake(SSLSocketImpl.java:426)",
];
const TLS_HANDSHAKE_OUTER: [&str; 6] = [
    "java.base/sun.net.www.protocol.https.HttpsClient.afterConnect(HttpsClient.java:483)",
    "java.base/sun.net.www.protocol.https.AbstractDelegateHttpsURLConnection.connect(AbstractDelegateHttpsURLConnection.java:190)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream0(HttpURLConnection.java:1377)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream(HttpURLConnection.java:1302)",
    "java.base/java.net.HttpURLConnection.getResponseCode(HttpURLConnection.java:493)",
    HTTPS_GET_RESPONSE_CODE,
];
const TLS_APP_READ: [&str; 4] = [
    "java.base/sun.security.ssl.SSLSocketInputRecord.bytesInCompletePacket(SSLSocketInputRecord.java:70)",
    "java.base/sun.security.ssl.SSLSocketImpl.readApplicationRecord(SSLSocketImpl.java:1461)",
    "java.base/sun.security.ssl.SSLSocketImpl$AppInputStream.read(SSLSocketImpl.java:1068)",
    "java.base/java.io.BufferedInputStream.fill(BufferedInputStream.java:289)",
];
const PLAIN_APP_READ: [&str; 1] = ["java.base/java.io.BufferedInputStream.fill(BufferedInputStream.java:289)"];
const BUFFERED_READ: [&str; 2] = ["java.base/java.io.BufferedInputStream.read1(BufferedInputStream.java:330)", "java.base/java.io.BufferedInputStream.read(BufferedInputStream.java:388)"];

const PARSE_HEADER_READ: &str = "java.base/sun.net.www.http.HttpClient.parseHTTPHeader(HttpClient.java:795)";
const PARSE_HTTP: &str = "java.base/sun.net.www.http.HttpClient.parseHTTP(HttpClient.java:727)";
const PARSE_HTTP_RETRY: &str = "java.base/sun.net.www.http.HttpClient.parseHTTP(HttpClient.java:754)";
const PARSE_HEADER_EOF_RETRY: &str = "java.base/sun.net.www.http.HttpClient.parseHTTPHeader(HttpClient.java:920)";
const PARSE_HEADER_EOF: &str = "java.base/sun.net.www.http.HttpClient.parseHTTPHeader(HttpClient.java:923)";
const GET_INPUT_STREAM_0_READ: &str = "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream0(HttpURLConnection.java:1392)";
const GET_INPUT_STREAM: &str = "java.base/sun.net.www.protocol.http.HttpURLConnection.getInputStream(HttpURLConnection.java:1302)";
const GET_RESPONSE_CODE: &str = "java.base/java.net.HttpURLConnection.getResponseCode(HttpURLConnection.java:493)";

const METERED_STREAM_READ: &str = "java.base/sun.net.www.MeteredStream.read(MeteredStream.java:134)";
const METERED_STREAM_JUST_READ: [&str; 2] = ["java.base/sun.net.www.MeteredStream.justRead(MeteredStream.java:58)", "java.base/sun.net.www.MeteredStream.read(MeteredStream.java:135)"];
const CHUNKED_FAST_READ: [&str; 2] = ["java.base/sun.net.www.http.ChunkedInputStream.fastRead(ChunkedInputStream.java:257)", "java.base/sun.net.www.http.ChunkedInputStream.read(ChunkedInputStream.java:698)"];
const BODY_READ_TAIL: [&str; 12] = [
    "java.base/java.io.FilterInputStream.read(FilterInputStream.java:119)",
    "java.base/sun.net.www.protocol.http.HttpURLConnection$HttpInputStream.read(HttpURLConnection.java:3322)",
    "java.base/sun.nio.cs.StreamDecoder.readBytes(StreamDecoder.java:279)",
    "java.base/sun.nio.cs.StreamDecoder.implRead(StreamDecoder.java:322)",
    "java.base/sun.nio.cs.StreamDecoder.read(StreamDecoder.java:186)",
    "java.base/java.io.InputStreamReader.read(InputStreamReader.java:183)",
    "java.base/java.io.Reader.read(Reader.java:372)",
    "org.apache.commons.io.IOUtils.copyLarge(IOUtils.java:1579)",
    "org.apache.commons.io.IOUtils.copyLarge(IOUtils.java:1558)",
    "org.apache.commons.io.IOUtils.copy(IOUtils.java:1355)",
    "org.apache.commons.io.IOUtils.copy(IOUtils.java:1128)",
    "org.apache.commons.io.IOUtils.toString(IOUtils.java:3134)",
];

fn body_site(status: u16) -> Vec<&'static str> {
    if status >= 400 { vec![BUILD_HTTP_EXCEPTION_164, READ_SERVICE_RESPONSE_147] } else { vec![READ_SERVICE_RESPONSE_141] }
}

fn body_read_tail(status: u16) -> Vec<&'static str> {
    BODY_READ_TAIL.iter().copied().chain(body_site(status)).collect()
}

fn concat(parts: &[&[&'static str]]) -> Vec<&'static str> {
    parts.iter().flat_map(|part| part.iter().copied()).collect()
}

fn windows_error_text(text: &str) -> String {
    let text = text.rsplit_once(" (os error").map_or(text, |(text, _)| text);
    text.trim_end_matches('.').to_string()
}

impl Failure {
    pub fn cause(&self, url: &JavaUrl) -> JavaThrowable {
        let https = url.is_https();
        let connect_frames: Vec<&'static str> = if https { HTTPS_CONNECT_FRAMES.to_vec() } else { HTTP_CONNECT_FRAMES.to_vec() };
        let connect_tail = |head: &[&'static str]| -> Vec<&'static str> {
            let mut frames = head.to_vec();
            frames.extend(connect_frames.iter().copied());
            if https {
                frames.push(HTTPS_GET_RESPONSE_CODE);
            }
            frames.push(READ_SERVICE_RESPONSE_130);
            frames
        };
        let handshake_frames = |head: &[&'static str], tail: &[&'static str]| -> Vec<&'static str> {
            let mut frames = head.to_vec();
            frames.extend(tail);
            frames.extend(TLS_HANDSHAKE_OUTER);
            frames.push(READ_SERVICE_RESPONSE_130);
            frames
        };
        match self {
            Failure::UnknownHost(host) => JavaThrowable::new(format!("java.net.UnknownHostException: {host}"), connect_tail(&["java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:569)"])),
            Failure::ConnectionRefused => JavaThrowable::new(
                format!("java.net.ConnectException: {}", connection_refused_message()),
                connect_tail(&[
                    "java.base/sun.nio.ch.Net.pollConnect(Native Method)",
                    "java.base/sun.nio.ch.Net.pollConnectNow(Net.java:639)",
                    "java.base/sun.nio.ch.NioSocketImpl.timedFinishConnect(NioSocketImpl.java:543)",
                    "java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:594)",
                ]),
            ),
            Failure::ConnectTimedOut => JavaThrowable::new(
                "java.net.SocketTimeoutException: Connect timed out",
                connect_tail(&["java.base/sun.nio.ch.NioSocketImpl.timedFinishConnect(NioSocketImpl.java:547)", "java.base/sun.nio.ch.NioSocketImpl.connect(NioSocketImpl.java:594)"]),
            ),
            Failure::Handshake(handshake) => match handshake {
                Handshake::ReadTimedOut => JavaThrowable::new(
                    "java.net.SocketTimeoutException: Read timed out",
                    handshake_frames(&concat(&[&SOCKET_READ_TIMED_OUT, &SOCKET_READ, &TLS_RECORD_READ]), &TLS_HANDSHAKE_DECODE),
                ),
                Handshake::ResetOnRead => {
                    JavaThrowable::new("java.net.SocketException: Connection reset", handshake_frames(&concat(&[&SOCKET_READ_RESET, &SOCKET_READ, &TLS_RECORD_READ]), &TLS_HANDSHAKE_DECODE))
                }
                Handshake::ResetOnWrite => JavaThrowable::new(
                    format!("java.net.SocketException: {}", connection_reset_by_peer_message()),
                    handshake_frames(
                        &concat(&[
                            &SOCKET_WRITE,
                            &[
                                "java.base/sun.security.ssl.SSLSocketOutputRecord.flush(SSLSocketOutputRecord.java:271)",
                                "java.base/sun.security.ssl.HandshakeOutStream.flush(HandshakeOutStream.java:89)",
                                "java.base/sun.security.ssl.ClientHello$ClientHelloKickstartProducer.produce(ClientHello.java:643)",
                                "java.base/sun.security.ssl.SSLHandshake.kickstart(SSLHandshake.java:554)",
                                "java.base/sun.security.ssl.ClientHandshakeContext.kickstart(ClientHandshakeContext.java:112)",
                                "java.base/sun.security.ssl.TransportContext.kickstart(TransportContext.java:256)",
                                "java.base/sun.security.ssl.SSLSocketImpl.startHandshake(SSLSocketImpl.java:448)",
                            ],
                        ]),
                        &["java.base/sun.security.ssl.SSLSocketImpl.startHandshake(SSLSocketImpl.java:426)"],
                    ),
                ),
                Handshake::Eof => {
                    let frames = handshake_frames(
                        &[
                            "java.base/sun.security.ssl.SSLSocketImpl.handleEOF(SSLSocketImpl.java:1716)",
                            "java.base/sun.security.ssl.SSLSocketImpl.decode(SSLSocketImpl.java:1514)",
                            "java.base/sun.security.ssl.SSLSocketImpl.readHandshakeRecord(SSLSocketImpl.java:1421)",
                        ],
                        &TLS_HANDSHAKE_DECODE[4..],
                    );
                    let eof_head = [
                        "java.base/sun.security.ssl.SSLSocketInputRecord.read(SSLSocketInputRecord.java:494)",
                        "java.base/sun.security.ssl.SSLSocketInputRecord.readHeader(SSLSocketInputRecord.java:483)",
                        "java.base/sun.security.ssl.SSLSocketInputRecord.decode(SSLSocketInputRecord.java:160)",
                        "java.base/sun.security.ssl.SSLTransport.decode(SSLTransport.java:111)",
                        "java.base/sun.security.ssl.SSLSocketImpl.decode(SSLSocketImpl.java:1506)",
                    ];
                    let eof_frames: Vec<&'static str> = eof_head.iter().copied().chain(frames[2..].iter().copied()).collect();
                    JavaThrowable::new("javax.net.ssl.SSLHandshakeException: Remote host terminated the handshake", frames).with_cause(JavaThrowable::new("java.io.EOFException: SSL peer shut down incorrectly", eof_frames))
                }
                Handshake::Unrecognized => JavaThrowable::new(
                    "javax.net.ssl.SSLException: Unsupported or unrecognized SSL message",
                    handshake_frames(
                        &[
                            "java.base/sun.security.ssl.SSLSocketInputRecord.handleUnknownRecord(SSLSocketInputRecord.java:462)",
                            "java.base/sun.security.ssl.SSLSocketInputRecord.decode(SSLSocketInputRecord.java:175)",
                            "java.base/sun.security.ssl.SSLTransport.decode(SSLTransport.java:111)",
                            "java.base/sun.security.ssl.SSLSocketImpl.decode(SSLSocketImpl.java:1506)",
                        ],
                        &TLS_HANDSHAKE_DECODE[3..],
                    ),
                ),
                Handshake::Alert(description) => JavaThrowable::new(
                    format!("javax.net.ssl.SSLHandshakeException: ({description}) Received fatal alert: {description}"),
                    handshake_frames(
                        &[
                            "java.base/sun.security.ssl.Alert.createSSLException(Alert.java:130)",
                            "java.base/sun.security.ssl.Alert.createSSLException(Alert.java:117)",
                            "java.base/sun.security.ssl.TransportContext.fatal(TransportContext.java:363)",
                            "java.base/sun.security.ssl.Alert$AlertConsumer.consume(Alert.java:299)",
                            "java.base/sun.security.ssl.TransportContext.dispatch(TransportContext.java:202)",
                            "java.base/sun.security.ssl.SSLTransport.decode(SSLTransport.java:172)",
                            "java.base/sun.security.ssl.SSLSocketImpl.decode(SSLSocketImpl.java:1506)",
                        ],
                        &TLS_HANDSHAKE_DECODE[3..],
                    ),
                ),
            },
            Failure::ReadTimedOut(phase) => JavaThrowable::new("java.net.SocketTimeoutException: Read timed out", self.data_read_frames(*phase, &SOCKET_READ_TIMED_OUT, https)),
            Failure::ConnectionReset(phase) => JavaThrowable::new("java.net.SocketException: Connection reset", self.data_read_frames(*phase, &SOCKET_READ_RESET, https)),
            Failure::ConnectionAborted(phase, text) => JavaThrowable::new(format!("java.net.SocketException: {text}"), self.data_read_frames(*phase, &SOCKET_READ_RESET, https)),
            Failure::UnexpectedEof => {
                let mut frames = vec![PARSE_HEADER_EOF, PARSE_HTTP, PARSE_HEADER_EOF_RETRY, PARSE_HTTP, GET_INPUT_STREAM_0_READ, GET_INPUT_STREAM, GET_RESPONSE_CODE];
                if https {
                    frames.push(HTTPS_GET_RESPONSE_CODE);
                }
                frames.push(READ_SERVICE_RESPONSE_130);
                JavaThrowable::new("java.net.SocketException: Unexpected end of file from server", frames)
            }
            Failure::PrematureEof { chunked, status } => {
                let head: &[&'static str] = if *chunked { &CHUNKED_FAST_READ } else { &METERED_STREAM_JUST_READ };
                JavaThrowable::new("java.io.IOException: Premature EOF", concat(&[head, &body_read_tail(*status)]))
            }
            Failure::Other(message) => JavaThrowable::new(format!("java.io.IOException: {message}"), Vec::new()),
        }
    }

    fn data_read_frames(&self, phase: Phase, socket_head: &[&'static str], https: bool) -> Vec<&'static str> {
        let transport: Vec<&'static str> = if https { concat(&[&SOCKET_READ, &TLS_RECORD_READ, &TLS_APP_READ]) } else { concat(&[&SOCKET_READ, &PLAIN_APP_READ]) };
        let reset = socket_head == SOCKET_READ_RESET;
        let mut frames = socket_head.to_vec();
        frames.extend(transport);
        frames.extend(BUFFERED_READ);
        match phase {
            Phase::Header => {
                frames.push(PARSE_HEADER_READ);
                frames.push(PARSE_HTTP);
                if reset {
                    frames.push(PARSE_HTTP_RETRY);
                }
                frames.extend([GET_INPUT_STREAM_0_READ, GET_INPUT_STREAM, GET_RESPONSE_CODE]);
                if https {
                    frames.push(HTTPS_GET_RESPONSE_CODE);
                }
                frames.push(READ_SERVICE_RESPONSE_130);
            }
            Phase::Body { status } => {
                frames.push(METERED_STREAM_READ);
                frames.extend(body_read_tail(status));
            }
        }
        frames
    }
}

pub fn connection_refused_message() -> &'static str {
    if cfg!(windows) { "Connection refused: getsockopt" } else { "Connection refused" }
}

pub fn connection_reset_by_peer_message() -> &'static str {
    "Connection reset by peer"
}

#[derive(Default)]
struct Recorder {
    state: Mutex<RecorderState>,
}

#[derive(Default)]
struct RecorderState {
    timed_out: bool,
    last_error: Option<(bool, io::ErrorKind, Option<i32>)>,
}

impl Recorder {
    fn record(&self, write: bool, error: &io::Error) {
        self.state.lock().unwrap().last_error = Some((write, error.kind(), error.raw_os_error()));
    }

    fn timed_out(&self) -> bool {
        std::mem::take(&mut self.state.lock().unwrap().timed_out)
    }

    fn take_error(&self) -> Option<(bool, io::ErrorKind, Option<i32>)> {
        self.state.lock().unwrap().last_error.take()
    }
}

struct Timed {
    inner: TcpStream,
    sleep: Option<Pin<Box<Sleep>>>,
    recorder: Arc<Recorder>,
}

impl AsyncRead for Timed {
    fn poll_read(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &mut ReadBuf<'_>) -> Poll<io::Result<()>> {
        let this = &mut *self;
        match Pin::new(&mut this.inner).poll_read(cx, buf) {
            Poll::Ready(result) => {
                this.sleep = None;
                if let Err(error) = &result {
                    this.recorder.record(false, error);
                }
                Poll::Ready(result)
            }
            Poll::Pending => {
                let sleep = this.sleep.get_or_insert_with(|| Box::pin(tokio::time::sleep(READ_TIMEOUT)));
                if sleep.as_mut().poll(cx).is_ready() {
                    this.sleep = None;
                    this.recorder.state.lock().unwrap().timed_out = true;
                    return Poll::Ready(Err(io::Error::new(io::ErrorKind::TimedOut, "Read timed out")));
                }
                Poll::Pending
            }
        }
    }
}

impl AsyncWrite for Timed {
    fn poll_write(mut self: Pin<&mut Self>, cx: &mut Context<'_>, buf: &[u8]) -> Poll<io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(cx, buf);
        if let Poll::Ready(Err(error)) = &result {
            self.recorder.record(true, error);
        }
        result
    }

    fn poll_flush(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        let result = Pin::new(&mut self.inner).poll_flush(cx);
        if let Poll::Ready(Err(error)) = &result {
            self.recorder.record(true, error);
        }
        result
    }

    fn poll_shutdown(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(cx)
    }
}

enum Connection {
    Plain(Timed),
    Tls(Box<tokio_rustls::client::TlsStream<Timed>>),
}

impl Connection {
    async fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        match self {
            Connection::Plain(stream) => stream.read(buffer).await,
            Connection::Tls(stream) => stream.read(buffer).await,
        }
    }

    async fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        match self {
            Connection::Plain(stream) => stream.write_all(bytes).await?,
            Connection::Tls(stream) => stream.write_all(bytes).await?,
        }
        match self {
            Connection::Plain(stream) => stream.flush().await,
            Connection::Tls(stream) => stream.flush().await,
        }
    }
}

fn tls_config() -> Arc<ClientConfig> {
    static CONFIG: OnceLock<Arc<ClientConfig>> = OnceLock::new();
    CONFIG
        .get_or_init(|| {
            let mut roots = RootCertStore::empty();
            match trust_store_certificates() {
                Some(certificates) => {
                    for certificate in certificates {
                        let _ = roots.add(certificate);
                    }
                }
                None => roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned()),
            }
            let provider = Arc::new(rustls::crypto::ring::default_provider());
            let config = ClientConfig::builder_with_provider(provider).with_safe_default_protocol_versions().expect("TLS protocol versions").with_root_certificates(roots).with_no_client_auth();
            Arc::new(config)
        })
        .clone()
}

fn trust_store_certificates() -> Option<Vec<CertificateDer<'static>>> {
    let path = crate::util::java_runtime::get_property("javax.net.ssl.trustStore")?;
    let password = crate::util::java_runtime::get_property("javax.net.ssl.trustStorePassword").unwrap_or_default();
    let data = std::fs::read(path).ok()?;
    let store = p12_keystore::KeyStore::from_pkcs12(&data, &password, p12_keystore::Pkcs12ImportPolicy::Raw).ok()?;
    Some(
        store
            .entries()
            .filter_map(|(_, entry)| match entry {
                p12_keystore::KeyStoreEntry::Certificate(certificate) => Some(CertificateDer::from(certificate.as_der().to_vec())),
                _ => None,
            })
            .collect(),
    )
}

fn alert_name(description: &rustls::AlertDescription) -> String {
    let debug = format!("{description:?}");
    if debug == "UnknownCA" {
        return "unknown_ca".to_string();
    }
    let mut name = String::new();
    for (index, c) in debug.chars().enumerate() {
        if c.is_ascii_uppercase() && index > 0 {
            name.push('_');
        }
        name.push(c.to_ascii_lowercase());
    }
    name
}

fn classify_handshake(error: &io::Error, recorder: &Recorder) -> Failure {
    if recorder.timed_out() {
        return Failure::Handshake(Handshake::ReadTimedOut);
    }
    if let Some((write, kind, _)) = recorder.take_error() {
        if matches!(kind, io::ErrorKind::ConnectionReset | io::ErrorKind::ConnectionAborted | io::ErrorKind::BrokenPipe) {
            return Failure::Handshake(if write { Handshake::ResetOnWrite } else { Handshake::ResetOnRead });
        }
    }
    if error.kind() == io::ErrorKind::UnexpectedEof {
        return Failure::Handshake(Handshake::Eof);
    }
    let rustls_error = error.get_ref().and_then(|inner| inner.downcast_ref::<rustls::Error>());
    match rustls_error {
        Some(rustls::Error::AlertReceived(description)) => Failure::Handshake(Handshake::Alert(alert_name(description))),
        Some(rustls::Error::InvalidMessage(_)) => Failure::Handshake(Handshake::Unrecognized),
        Some(rustls::Error::InvalidCertificate(_)) => Failure::Other(
            "PKIX path building failed: sun.security.provider.certpath.SunCertPathBuilderException: unable to find valid certification path to requested target".to_string(),
        ),
        _ => Failure::Other(error.to_string()),
    }
}

async fn connect(url: &JavaUrl) -> Result<Connection, Failure> {
    let host = url.host.clone();
    let port = url.effective_port();
    let addresses: Vec<std::net::SocketAddr> = match tokio::net::lookup_host((host.as_str(), port)).await {
        Ok(addresses) => addresses.collect(),
        Err(_) => return Err(Failure::UnknownHost(url.host.clone())),
    };
    let address = addresses.iter().find(|address| address.is_ipv4()).or(addresses.first()).copied().ok_or_else(|| Failure::UnknownHost(host.clone()))?;
    let stream = match tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(address)).await {
        Err(_) => return Err(Failure::ConnectTimedOut),
        Ok(Err(error)) if error.kind() == io::ErrorKind::ConnectionRefused => return Err(Failure::ConnectionRefused),
        Ok(Err(error)) => return Err(Failure::Other(error.to_string())),
        Ok(Ok(stream)) => stream,
    };
    let _ = stream.set_nodelay(true);
    let recorder = Arc::new(Recorder::default());
    let timed = Timed { inner: stream, sleep: None, recorder: recorder.clone() };
    if !url.is_https() {
        return Ok(Connection::Plain(timed));
    }
    let server_name = match ServerName::try_from(host.clone()) {
        Ok(name) => name,
        Err(_) => return Err(Failure::UnknownHost(host)),
    };
    match TlsConnector::from(tls_config()).connect(server_name, timed).await {
        Ok(stream) => {
            recorder.take_error();
            Ok(Connection::Tls(Box::new(stream)))
        }
        Err(error) => Err(classify_handshake(&error, &recorder)),
    }
}

struct Reader {
    connection: Connection,
    buffer: Vec<u8>,
    start: usize,
}

enum ReadError {
    Timeout,
    Reset,
    Aborted(String),
    Other(String),
}

impl Reader {
    async fn fill(&mut self) -> Result<usize, ReadError> {
        if self.start > 0 && self.start == self.buffer.len() {
            self.buffer.clear();
            self.start = 0;
        }
        let mut chunk = [0u8; 8192];
        let result = self.connection.read(&mut chunk).await;
        let recorder = match &self.connection {
            Connection::Plain(stream) => stream.recorder.clone(),
            Connection::Tls(stream) => stream.get_ref().0.recorder.clone(),
        };
        match result {
            Ok(count) => {
                self.buffer.extend_from_slice(&chunk[..count]);
                Ok(count)
            }
            Err(error) => {
                if recorder.timed_out() {
                    return Err(ReadError::Timeout);
                }
                let recorded = recorder.take_error();
                let kind = recorded.map_or(error.kind(), |(_, kind, _)| kind);
                let raw = recorded.and_then(|(_, _, raw)| raw).or(error.raw_os_error());
                match kind {
                    io::ErrorKind::ConnectionReset => Err(ReadError::Reset),
                    io::ErrorKind::ConnectionAborted => Err(ReadError::Aborted(match raw {
                        Some(code) => windows_error_text(&io::Error::from_raw_os_error(code).to_string()),
                        None => "Software caused connection abort".to_string(),
                    })),
                    io::ErrorKind::UnexpectedEof => Ok(0),
                    _ => Err(ReadError::Other(error.to_string())),
                }
            }
        }
    }

    fn available(&self) -> &[u8] {
        &self.buffer[self.start..]
    }

    async fn read_line(&mut self) -> Result<Option<String>, ReadError> {
        loop {
            if let Some(position) = self.available().iter().position(|&byte| byte == b'\n') {
                let line = String::from_utf8_lossy(&self.available()[..position]).trim_end_matches('\r').to_string();
                self.start += position + 1;
                return Ok(Some(line));
            }
            if self.fill().await? == 0 {
                self.start = self.buffer.len();
                return Ok(None);
            }
        }
    }
}

fn header_failure(error: ReadError) -> Failure {
    match error {
        ReadError::Timeout => Failure::ReadTimedOut(Phase::Header),
        ReadError::Reset => Failure::ConnectionReset(Phase::Header),
        ReadError::Aborted(text) => Failure::ConnectionAborted(Phase::Header, text),
        ReadError::Other(message) => Failure::Other(message),
    }
}

fn body_failure(error: ReadError, status: u16) -> Failure {
    match error {
        ReadError::Timeout => Failure::ReadTimedOut(Phase::Body { status }),
        ReadError::Reset => Failure::ConnectionReset(Phase::Body { status }),
        ReadError::Aborted(text) => Failure::ConnectionAborted(Phase::Body { status }, text),
        ReadError::Other(message) => Failure::Other(message),
    }
}

fn request_bytes(method: Method, url: &JavaUrl, body: Option<&[u8]>) -> Vec<u8> {
    let mut head = format!(
        "{} {} HTTP/1.1\r\nCache-Control: no-cache\r\nPragma: no-cache\r\nUser-Agent: Java/{}\r\nHost: {}\r\nAccept: */*\r\nConnection: keep-alive\r\n",
        method.name(),
        url.request_target(),
        crate::util::java_runtime::java_version(),
        url.host_header()
    );
    if let Some(body) = body {
        head.push_str(&format!("Content-Type: application/json; charset=utf-8\r\nContent-Length: {}\r\n", body.len()));
    }
    head.push_str("\r\n");
    let mut bytes = head.into_bytes();
    if let Some(body) = body {
        bytes.extend_from_slice(body);
    }
    bytes
}

struct Head {
    status: u16,
    content_type: Option<String>,
    content_length: Option<usize>,
    chunked: bool,
}

async fn read_head(reader: &mut Reader) -> Result<Head, Failure> {
    let status_line = match reader.read_line().await.map_err(header_failure)? {
        Some(line) => line,
        None => return Err(Failure::UnexpectedEof),
    };
    let mut parts = status_line.splitn(3, ' ');
    let version = parts.next().unwrap_or_default();
    let status = parts.next().and_then(|code| code.trim().parse::<u16>().ok());
    let (true, Some(status)) = (version.starts_with("HTTP/"), status) else { return Err(Failure::Other(format!("Invalid Http response"))) };
    let mut head = Head { status, content_type: None, content_length: None, chunked: false };
    loop {
        match reader.read_line().await.map_err(header_failure)? {
            None => break,
            Some(line) if line.is_empty() => break,
            Some(line) => {
                if let Some((name, value)) = line.split_once(':') {
                    let value = value.trim();
                    match name.trim().to_ascii_lowercase().as_str() {
                        "content-type" => head.content_type = Some(value.to_string()),
                        "content-length" => head.content_length = value.parse().ok(),
                        "transfer-encoding" => head.chunked = value.eq_ignore_ascii_case("chunked"),
                        _ => {}
                    }
                }
            }
        }
    }
    Ok(head)
}

async fn read_body(reader: &mut Reader, head: &Head) -> Result<Vec<u8>, Failure> {
    let status = head.status;
    if status == 204 || status == 304 || (100..200).contains(&status) {
        return Ok(Vec::new());
    }
    let mut body = Vec::new();
    if head.chunked {
        loop {
            let size_line = reader.read_line().await.map_err(|error| body_failure(error, status))?.ok_or(Failure::PrematureEof { chunked: true, status })?;
            let size = usize::from_str_radix(size_line.split(';').next().unwrap_or("").trim(), 16).map_err(|_| Failure::Other("Bogus chunk size".to_string()))?;
            if size == 0 {
                loop {
                    match reader.read_line().await.map_err(|error| body_failure(error, status))? {
                        None => break,
                        Some(line) if line.is_empty() => break,
                        Some(_) => {}
                    }
                }
                return Ok(body);
            }
            let mut remaining = size;
            while remaining > 0 {
                if reader.available().is_empty() && reader.fill().await.map_err(|error| body_failure(error, status))? == 0 {
                    return Err(Failure::PrematureEof { chunked: true, status });
                }
                let take = remaining.min(reader.available().len());
                body.extend_from_slice(&reader.available()[..take]);
                reader.start += take;
                remaining -= take;
            }
            reader.read_line().await.map_err(|error| body_failure(error, status))?;
        }
    }
    match head.content_length {
        Some(length) => {
            while body.len() < length {
                if reader.available().is_empty() && reader.fill().await.map_err(|error| body_failure(error, status))? == 0 {
                    return Err(Failure::PrematureEof { chunked: false, status });
                }
                let take = (length - body.len()).min(reader.available().len());
                body.extend_from_slice(&reader.available()[..take]);
                reader.start += take;
            }
        }
        None => loop {
            body.extend_from_slice(reader.available());
            reader.start = reader.buffer.len();
            if reader.fill().await.map_err(|error| body_failure(error, status))? == 0 {
                break;
            }
        },
    }
    Ok(body)
}

pub async fn send(method: Method, url: &JavaUrl, body: Option<&[u8]>) -> Result<Response, Failure> {
    let request = request_bytes(method, url, body);
    let mut failed_once = false;
    loop {
        let connection = connect(url).await?;
        let mut reader = Reader { connection, buffer: Vec::new(), start: 0 };
        let _ = reader.connection.write_all(&request).await;
        match read_head(&mut reader).await {
            Ok(head) => {
                let body = read_body(&mut reader, &head).await?;
                return Ok(Response { status: head.status, content_type: head.content_type, body });
            }
            Err(Failure::UnexpectedEof | Failure::ConnectionReset(Phase::Header) | Failure::ConnectionAborted(Phase::Header, _)) if !failed_once => failed_once = true,
            Err(failure) => return Err(failure),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    async fn serve(behaviour: impl Fn(tokio::net::TcpStream) + Send + Sync + 'static) -> (u16, Arc<AtomicUsize>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let accepted = Arc::new(AtomicUsize::new(0));
        let counter = accepted.clone();
        tokio::spawn(async move {
            while let Ok((stream, _)) = listener.accept().await {
                counter.fetch_add(1, Ordering::SeqCst);
                behaviour(stream);
            }
        });
        (port, accepted)
    }

    fn reset(stream: tokio::net::TcpStream) {
        let stream = stream.into_std().unwrap();
        socket2::SockRef::from(&stream).set_linger(Some(Duration::ZERO)).unwrap();
        drop(stream);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_silent_server_during_the_tls_handshake_is_a_read_timeout() {
        let (port, _) = serve(|stream| {
            tokio::spawn(async move {
                let _stream = stream;
                tokio::time::sleep(Duration::from_secs(30)).await;
            });
        })
        .await;
        let url = JavaUrl::parse(&format!("https://127.0.0.1:{port}/")).unwrap();
        let started = std::time::Instant::now();
        let failure = send(Method::Get, &url, None).await.unwrap_err();
        assert_eq!(failure, Failure::Handshake(Handshake::ReadTimedOut));
        assert!(started.elapsed() >= READ_TIMEOUT && started.elapsed() < READ_TIMEOUT + Duration::from_secs(3));
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_reset_while_waiting_for_the_response_is_retried_once_and_reported_as_connection_reset() {
        let (port, accepted) = serve(|stream| {
            tokio::spawn(async move {
                let mut stream = stream;
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer).await;
                reset(stream);
            });
        })
        .await;
        let url = JavaUrl::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        let failure = send(Method::Get, &url, None).await.unwrap_err();
        assert_eq!(failure, Failure::ConnectionReset(Phase::Header));
        assert_eq!(accepted.load(Ordering::SeqCst), 2);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_reset_during_the_tls_handshake_is_a_reset_on_read_or_write() {
        let (port, _) = serve(reset).await;
        let url = JavaUrl::parse(&format!("https://127.0.0.1:{port}/")).unwrap();
        let failure = send(Method::Get, &url, None).await.unwrap_err();
        assert!(matches!(failure, Failure::Handshake(Handshake::ResetOnRead | Handshake::ResetOnWrite)), "{failure:?}");
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn a_truncated_body_is_a_premature_eof_and_a_partial_header_is_an_empty_response() {
        let (port, _) = serve(|stream| {
            tokio::spawn(async move {
                let mut stream = stream;
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer).await;
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n{\"a\":").await;
            });
        })
        .await;
        let url = JavaUrl::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        assert_eq!(send(Method::Get, &url, None).await.unwrap_err(), Failure::PrematureEof { chunked: false, status: 200 });
        let (port, _) = serve(|stream| {
            tokio::spawn(async move {
                let mut stream = stream;
                let mut buffer = [0u8; 1024];
                let _ = stream.read(&mut buffer).await;
                let _ = stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Ty").await;
            });
        })
        .await;
        let url = JavaUrl::parse(&format!("http://127.0.0.1:{port}/")).unwrap();
        let response = send(Method::Get, &url, None).await.unwrap();
        assert_eq!((response.status, response.body.len()), (200, 0));
    }

    #[test]
    fn connection_resets_use_the_text_of_the_bundled_runtime() {
        assert_eq!(Failure::ConnectionReset(Phase::Header).cause(&JavaUrl::parse("http://h/").unwrap()).description, "java.net.SocketException: Connection reset");
        assert_eq!(Failure::Handshake(Handshake::ResetOnWrite).cause(&JavaUrl::parse("https://h/").unwrap()).description, "java.net.SocketException: Connection reset by peer");
        assert_eq!(windows_error_text("An established connection was aborted by the software in your host machine. (os error 10053)"), "An established connection was aborted by the software in your host machine");
    }
}
