pub mod network_data_output_stream;
pub mod query_thread_gs4;
pub mod rcon_thread;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

static UNIQUE_THREAD_ID: AtomicUsize = AtomicUsize::new(0);
static LIVE_THREADS: Mutex<Vec<Arc<GenericThread>>> = Mutex::new(Vec::new());

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GenericThreadKind {
    QueryListener,
    RconListener,
    RconClient,
}

pub struct GenericThread {
    pub name: String,
    pub id: u32,
    pub kind: GenericThreadKind,
    running_command: AtomicBool,
}

impl GenericThread {
    pub fn is_running_command(&self) -> bool {
        self.running_command.load(Ordering::Relaxed)
    }

    pub fn set_running_command(&self, running: bool) {
        self.running_command.store(running, Ordering::Relaxed);
    }

    pub fn finish(&self) {
        LIVE_THREADS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).retain(|thread| !std::ptr::eq(&**thread, self));
    }
}

pub fn start_generic_thread(starter: &str, name: &str, kind: GenericThreadKind) -> Arc<GenericThread> {
    let thread = Arc::new(GenericThread {
        name: format!("{name} #{}", UNIQUE_THREAD_ID.fetch_add(1, Ordering::Relaxed) + 1),
        id: crate::util::next_java_thread_id(),
        kind,
        running_command: AtomicBool::new(false),
    });
    LIVE_THREADS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).push(thread.clone());
    info!(thread: starter, "Thread {name} started");
    thread
}

pub fn live_generic_threads() -> Vec<Arc<GenericThread>> {
    LIVE_THREADS.lock().unwrap_or_else(std::sync::PoisonError::into_inner).clone()
}

pub fn java_inet_address(address: std::net::IpAddr) -> String {
    match address {
        std::net::IpAddr::V4(v4) => format!("/{v4}"),
        std::net::IpAddr::V6(v6) => format!("/{}", v6.segments().map(|s| format!("{s:x}")).join(":")),
    }
}

pub fn java_bind_exception(error: &std::io::Error, datagram: bool) -> String {
    let reason = match error.kind() {
        std::io::ErrorKind::AddrInUse => "Address already in use",
        std::io::ErrorKind::AddrNotAvailable => "Cannot assign requested address",
        std::io::ErrorKind::PermissionDenied => "Permission denied",
        _ => return format!("java.net.SocketException: {error}"),
    };
    match (cfg!(windows), datagram) {
        (true, true) => format!("java.net.BindException: {reason}: Cannot bind"),
        (true, false) => format!("java.net.BindException: {reason}: bind"),
        (false, _) => format!("java.net.BindException: {reason}"),
    }
}
