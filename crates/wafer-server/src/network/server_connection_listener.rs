use std::net::SocketAddr;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, RwLock};

use anyhow::bail;
use wafer_data::packets::{handshake, login, status};
use wafer_data::{PROTOCOL_VERSION, VERSION_NAME};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::mpsc;

use crate::ServerConfig;
use crate::network::chat::{Arg, Component};
use crate::network::configuration::CommonListenerCookie;
use crate::network::connection::{self, PacketReader, PacketWriter};
use crate::network::encryption::ServerKeyPair;
use crate::server::Event;
use crate::server::notifications::server_activity_monitor::ServerActivityMonitor;
use crate::server::services::Services;

static NEXT_CONN_ID: AtomicU64 = AtomicU64::new(1);

pub struct ListenerSettings {
    replies_to_status: AtomicBool,
    accepts_transfers: AtomicBool,
}

impl ListenerSettings {
    pub fn new(replies_to_status: bool, accepts_transfers: bool) -> ListenerSettings {
        ListenerSettings { replies_to_status: AtomicBool::new(replies_to_status), accepts_transfers: AtomicBool::new(accepts_transfers) }
    }

    pub fn replies_to_status(&self) -> bool {
        self.replies_to_status.load(Ordering::Relaxed)
    }

    pub fn set_replies_to_status(&self, value: bool) {
        self.replies_to_status.store(value, Ordering::Relaxed);
    }

    pub fn accepts_transfers(&self) -> bool {
        self.accepts_transfers.load(Ordering::Relaxed)
    }

    pub fn set_accepts_transfers(&self, value: bool) {
        self.accepts_transfers.store(value, Ordering::Relaxed);
    }
}

pub struct ListenerContext {
    pub config: ServerConfig,
    pub keys: Arc<ServerKeyPair>,
    pub services: Services,
    pub status: Arc<RwLock<Option<Arc<str>>>>,
    pub player_names_array: Arc<RwLock<Arc<[String]>>>,
    pub events: mpsc::UnboundedSender<Event>,
    pub settings: Arc<ListenerSettings>,
    pub server_activity_monitor: Arc<ServerActivityMonitor>,
}

fn event_loop_type(use_native_transport: bool) -> &'static str {
    if use_native_transport && cfg!(target_os = "macos") && cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
        "Kqueue"
    } else if use_native_transport && cfg!(target_os = "linux") && cfg!(any(target_arch = "x86_64", target_arch = "aarch64")) {
        "Epoll"
    } else {
        "NIO"
    }
}

fn somaxconn() -> i32 {
    if cfg!(windows) {
        return 200;
    }
    std::fs::read_to_string("/proc/sys/net/core/somaxconn").ok().and_then(|value| value.trim().parse().ok()).unwrap_or(128)
}

fn bind(address: SocketAddr, dual_stack: bool) -> std::io::Result<TcpListener> {
    let socket = socket2::Socket::new(socket2::Domain::for_address(address), socket2::Type::STREAM, Some(socket2::Protocol::TCP))?;
    if dual_stack {
        socket.set_only_v6(false)?;
    }
    if cfg!(unix) {
        socket.set_reuse_address(true)?;
    }
    socket.bind(&address.into())?;
    socket.listen(somaxconn())?;
    socket.set_nonblocking(true)?;
    TcpListener::from_std(socket.into())
}

pub fn start_tcp_server_listener(address: Option<std::net::IpAddr>, port: u16) -> std::io::Result<TcpListener> {
    match address {
        Some(address) => bind(SocketAddr::new(address, port), false),
        None => match socket2::Socket::new(socket2::Domain::IPV6, socket2::Type::STREAM, Some(socket2::Protocol::TCP)) {
            Ok(_) => bind(SocketAddr::new(std::net::Ipv6Addr::UNSPECIFIED.into(), port), true),
            Err(_) => bind(SocketAddr::new(std::net::Ipv4Addr::UNSPECIFIED.into(), port), false),
        },
    }
}

fn remote_address(address: SocketAddr) -> SocketAddr {
    match address {
        SocketAddr::V6(v6) => match v6.ip().to_ipv4_mapped() {
            Some(v4) => SocketAddr::new(v4.into(), v6.port()),
            None => address,
        },
        SocketAddr::V4(_) => address,
    }
}

pub async fn accept_connections(listener: TcpListener, context: Arc<ListenerContext>) {
    let event_loop_type = event_loop_type(context.config.use_native_transport);
    let event_loops = std::thread::available_parallelism().map_or(1, |n| n.get()) * 2;
    let mut channels = 0;
    loop {
        let Ok((stream, _)) = listener.accept().await else {
            tokio::time::sleep(std::time::Duration::from_secs(1)).await;
            continue;
        };
        channels += 1;
        let io_thread: Arc<str> = Arc::from(format!("Netty {event_loop_type} IO #{}", channels % event_loops));
        let context = context.clone();
        tokio::spawn(async move {
            let _ = handle_connection(stream, &context, io_thread).await;
        });
    }
}

async fn handle_connection(stream: TcpStream, context: &ListenerContext, io_thread: Arc<str>) -> anyhow::Result<()> {
    let address = remote_address(stream.peer_addr()?);
    let (mut r, mut w) = connection::split(stream)?;
    if context.settings.replies_to_status() && super::legacy_query_handler::handle(&mut r, &mut w, context).await? {
        return Ok(());
    }
    let packet = r.expect(handshake::serverbound::INTENTION).await?;
    let mut hr = packet.reader();
    let protocol = hr.varint()?;
    let _host = hr.string(255)?;
    let _port = hr.u16()?;
    match hr.varint()? {
        1 => {
            let status = context.status.read().unwrap().clone();
            match status {
                Some(status) if context.settings.replies_to_status() => status_phase(r, w, &status).await,
                _ => Ok(()),
            }
        }
        2 => begin_login(r, w, context, address, protocol, false, io_thread).await,
        3 if !context.settings.accepts_transfers() => {
            let reason = Component::translatable("multiplayer.disconnect.transfers_disabled");
            w.send(login::clientbound::LOGIN_DISCONNECT, |p| p.string(&reason.to_json())).await?;
            Ok(())
        }
        3 => begin_login(r, w, context, address, protocol, true, io_thread).await,
        intent => bail!("unknown handshake intent {intent}"),
    }
}

async fn begin_login(r: PacketReader, mut w: PacketWriter, context: &ListenerContext, address: SocketAddr, protocol: i32, transferred: bool, io_thread: Arc<str>) -> anyhow::Result<()> {
    if protocol != PROTOCOL_VERSION {
        let key = if protocol < 754 { "multiplayer.disconnect.outdated_client" } else { "multiplayer.disconnect.incompatible" };
        let reason = Component::translatable_with(key, vec![Arg::String(VERSION_NAME.to_string())]);
        w.send(login::clientbound::LOGIN_DISCONNECT, |p| p.string(&reason.to_json())).await?;
        return Ok(());
    }
    login_phase(r, w, context, address, transferred, io_thread).await
}

async fn login_phase(mut r: PacketReader, mut w: PacketWriter, context: &ListenerContext, address: SocketAddr, transferred: bool, io_thread: Arc<str>) -> anyhow::Result<()> {
    let events = &context.events;
    let Some(profile) = super::login::login(&mut r, &mut w, context, address).await? else {
        return Ok(());
    };
    let cookie = CommonListenerCookie::create_initial(transferred);
    let Some(cookie) = super::configuration::configure(&mut r, &mut w, &context.config, &profile, events, cookie).await else {
        return Ok(());
    };
    let conn = NEXT_CONN_ID.fetch_add(1, Ordering::Relaxed);
    let packets = events.clone();
    let out = connection::spawn_io(r, w, io_thread, move |packet| {
        let _ = packets.send(match packet {
            Ok(packet) => Event::Packet { conn, packet },
            Err(reason) => Event::Left { conn, reason },
        });
    });
    events.send(Event::Join { conn, profile, cookie, out, address })?;
    Ok(())
}

async fn status_phase(mut r: PacketReader, mut w: PacketWriter, response: &str) -> anyhow::Result<()> {
    let mut has_requested_status = false;
    loop {
        let packet = r.recv().await?;
        match packet.id {
            status::serverbound::STATUS_REQUEST if has_requested_status => return Ok(()),
            status::serverbound::STATUS_REQUEST => {
                has_requested_status = true;
                w.send(status::clientbound::STATUS_RESPONSE, |p| p.string(response)).await?;
            }
            status::serverbound::PING_REQUEST => {
                let payload = packet.reader().i64()?;
                w.send(status::clientbound::PONG_RESPONSE, |p| p.i64(payload)).await?;
                return Ok(());
            }
            id => bail!("unexpected status packet 0x{id:02x}"),
        }
    }
}

pub fn loggable_address(address: SocketAddr, log_ips: bool) -> String {
    if !log_ips {
        return "IP hidden".into();
    }
    match address {
        SocketAddr::V4(v4) => format!("/{}:{}", v4.ip(), v4.port()),
        SocketAddr::V6(v6) => {
            let segments = v6.ip().segments().map(|s| format!("{s:x}")).join(":");
            format!("/[{segments}]:{}", v6.port())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_wildcard_listener_accepts_ipv4_and_ipv6_like_java() {
        let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let _guard = runtime.enter();
        let listener = start_tcp_server_listener(None, 0).unwrap();
        let port = listener.local_addr().unwrap().port();
        for target in ["127.0.0.1", "::1"] {
            let target: std::net::IpAddr = target.parse().unwrap();
            let client = std::net::TcpStream::connect((target, port)).unwrap();
            let (stream, _) = runtime.block_on(listener.accept()).unwrap();
            assert_eq!(remote_address(stream.peer_addr().unwrap()), client.local_addr().unwrap());
        }
    }
}
