pub mod dedicated_server;
pub mod dedicated_server_properties;
pub mod dedicated_server_settings;
pub mod server_watchdog;
pub mod settings;

use std::io::{self, BufRead};
use std::path::Path;

use tokio::sync::mpsc;

use super::Event;
use super::server_links::{Entry, KnownLinkType};
use dedicated_server_properties::DedicatedServerProperties;

pub fn create_server_links(properties: &DedicatedServerProperties) -> Vec<Entry> {
    parse_bug_report_link(properties).map(|link| KnownLinkType::BugReport.create(link)).into_iter().collect()
}

fn parse_bug_report_link(properties: &DedicatedServerProperties) -> Option<crate::util::uri::URI> {
    let bug_report_link = &properties.bug_report_link;
    if bug_report_link.is_empty() {
        return None;
    }
    match crate::util::parse_and_validate_untrusted_uri(bug_report_link) {
        Ok(link) => Some(link),
        Err(e) => {
            warn!("Failed to parse bug link {bug_report_link}\njava.net.URISyntaxException: {e}");
            None
        }
    }
}

pub fn read_code_of_conducts() -> Result<Vec<(String, String)>, String> {
    let path = Path::new("codeofconduct");
    if !std::fs::symlink_metadata(path).is_ok_and(|m| m.is_dir()) {
        return Err(format!("java.lang.IllegalArgumentException: Code of Conduct folder does not exist: {}", path.display()));
    }
    let folder_error = |e: io::Error| format!("java.lang.IllegalArgumentException: Failed to read Code of Conduct folder\nCaused by: {}", java_io_exception(&e));
    let allowed = std::path::absolute(path).map_err(folder_error)?;
    let mut code_of_conducts: Vec<(String, String)> = Vec::new();
    for file in std::fs::read_dir(path).map_err(folder_error)? {
        let file = file.map_err(folder_error)?;
        let filename = file.file_name().to_string_lossy().into_owned();
        let Some(language) = filename.strip_suffix(".txt") else { continue };
        let language = language.to_lowercase();
        let real_parent = std::fs::canonicalize(file.path()).map_err(folder_error)?.parent().map(strip_verbatim);
        if real_parent.as_deref() != Some(allowed.as_path()) {
            return Err(format!(
                "java.lang.IllegalArgumentException: Failed to read Code of Conduct file \"{filename}\" because it links to a file outside the allowed directory"
            ));
        }
        let text = std::fs::read(file.path())
            .map_err(|e| java_io_exception(&e))
            .and_then(|bytes| String::from_utf8(bytes).map_err(|_| "java.nio.charset.MalformedInputException: Input length = 1".to_string()))
            .map_err(|cause| format!("java.lang.IllegalArgumentException: Failed to read Code of Conduct file {filename}\nCaused by: {cause}"))?;
        let code_of_conduct = crate::util::string_util::strip_color(&read_all_lines(&text).join("\n"));
        if let Some((_, existing)) = code_of_conducts.iter().find(|(l, _)| *l == language) {
            return Err(format!("java.lang.IllegalArgumentException: Multiple entries with same key: {language}={code_of_conduct} and {language}={existing}"));
        }
        code_of_conducts.push((language, code_of_conduct));
    }
    Ok(code_of_conducts)
}

fn read_all_lines(text: &str) -> Vec<&str> {
    let mut lines = Vec::new();
    let mut rest = text;
    while !rest.is_empty() {
        match rest.find(['\n', '\r']) {
            Some(i) => {
                lines.push(&rest[..i]);
                let skip = if rest[i..].starts_with("\r\n") { 2 } else { 1 };
                rest = &rest[i + skip..];
            }
            None => {
                lines.push(rest);
                break;
            }
        }
    }
    lines
}

fn strip_verbatim(path: &Path) -> std::path::PathBuf {
    let s = path.to_string_lossy();
    match s.strip_prefix(r"\\?\UNC\") {
        Some(unc) => format!(r"\\{unc}").into(),
        None => s.strip_prefix(r"\\?\").map_or_else(|| path.to_path_buf(), Into::into),
    }
}

fn java_io_exception(e: &io::Error) -> String {
    format!("java.io.IOException: {e}")
}

static SHUTDOWN_HOOK: std::sync::OnceLock<(mpsc::UnboundedSender<Event>, std::sync::Mutex<bool>, std::sync::Condvar)> = std::sync::OnceLock::new();

static EXIT_STATUS: std::sync::atomic::AtomicI32 = std::sync::atomic::AtomicI32::new(0);

pub fn exit_status() -> i32 {
    EXIT_STATUS.load(std::sync::atomic::Ordering::Acquire)
}

pub(crate) fn begin_shutdown(status: i32) {
    EXIT_STATUS.store(status, std::sync::atomic::Ordering::Release);
    crate::logging::stop();
}

pub fn add_shutdown_hook(events: mpsc::UnboundedSender<Event>) {
    let _ = SHUTDOWN_HOOK.set((events.clone(), std::sync::Mutex::new(false), std::sync::Condvar::new()));
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
    }
    #[cfg(unix)]
    for (kind, status) in [(tokio::signal::unix::SignalKind::terminate(), 143), (tokio::signal::unix::SignalKind::hangup(), 129)] {
        if let Ok(mut signal) = tokio::signal::unix::signal(kind) {
            let events = events.clone();
            tokio::spawn(async move {
                if signal.recv().await.is_some() {
                    begin_shutdown(status);
                    let _ = events.send(Event::Halt);
                }
            });
        }
    }
}

pub fn server_stopped() {
    if let Some((_, stopped, condition)) = SHUTDOWN_HOOK.get() {
        *stopped.lock().unwrap() = true;
        condition.notify_all();
    }
}

pub(crate) fn halt_and_wait() {
    if let Some((events, stopped, condition)) = SHUTDOWN_HOOK.get() {
        let _ = events.send(Event::Halt);
        let mut stopped = stopped.lock().unwrap();
        while !*stopped {
            stopped = condition.wait(stopped).unwrap();
        }
    }
}

#[cfg(windows)]
unsafe extern "system" fn console_ctrl_handler(ctrl_type: u32) -> windows_sys::core::BOOL {
    use windows_sys::Win32::System::Console::{CTRL_CLOSE_EVENT, CTRL_LOGOFF_EVENT, CTRL_SHUTDOWN_EVENT};
    match ctrl_type {
        CTRL_CLOSE_EVENT | CTRL_LOGOFF_EVENT | CTRL_SHUTDOWN_EVENT => {
            begin_shutdown(143);
            halt_and_wait();
            1
        }
        _ => 0,
    }
}

static OVERWORLD_CREATED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_overworld_created() {
    OVERWORLD_CREATED.store(true, std::sync::atomic::Ordering::Release);
}

pub fn start_console_thread(events: mpsc::UnboundedSender<Event>) {
    let _ = std::thread::Builder::new().name("Server console handler".into()).spawn(move || {
        let stdin = io::stdin();
        let mut line = String::new();
        loop {
            line.clear();
            match stdin.lock().read_line(&mut line) {
                Ok(0) => return,
                Ok(_) => {
                    let command = line.trim_end_matches(['\n', '\r']).to_string();
                    let has_level = OVERWORLD_CREATED.load(std::sync::atomic::Ordering::Acquire);
                    if events.send(Event::Console(command, has_level)).is_err() {
                        return;
                    }
                }
                Err(_) => {
                    error!("Exception handling console input");
                    return;
                }
            }
        }
    });
}

const GET_BY_NAME_FRAMES: [&str; 12] = [
    "java.base/java.net.Inet6AddressImpl.lookupAllHostAddr(Native Method)",
    "java.base/java.net.Inet6AddressImpl.lookupAllHostAddr(Inet6AddressImpl.java:52)",
    "java.base/java.net.InetAddress$PlatformResolver.lookupByName(InetAddress.java:1134)",
    "java.base/java.net.InetAddress.getAddressesFromNameService(InetAddress.java:1712)",
    "java.base/java.net.InetAddress$NameServiceAddresses.get(InetAddress.java:1060)",
    "java.base/java.net.InetAddress.getAllByName0(InetAddress.java:1702)",
    "java.base/java.net.InetAddress.getAllByName(InetAddress.java:1582)",
    "java.base/java.net.InetAddress.getByName(InetAddress.java:1492)",
    "net.minecraft.server.dedicated.DedicatedServer.initServer(DedicatedServer.java:181)",
    crate::server::RUN_SERVER_FRAME,
    crate::server::SPIN_FRAME,
    crate::server::THREAD_RUN_FRAME,
];

pub fn get_by_name(host: &str) -> Result<std::net::IpAddr, crate::util::java_throwable::JavaThrowable> {
    let literal = host.strip_prefix('[').and_then(|h| h.strip_suffix(']')).unwrap_or(host);
    if let Ok(address) = literal.parse() {
        return Ok(address);
    }
    let lookup = std::net::ToSocketAddrs::to_socket_addrs(&(host, 0)).map(|mut addresses| addresses.next());
    match lookup {
        Ok(Some(address)) => Ok(address.ip()),
        Ok(None) => Err(unknown_host_exception(host, None)),
        Err(error) => Err(unknown_host_exception(host, Some(&error))),
    }
}

fn unknown_host_exception(host: &str, error: Option<&io::Error>) -> crate::util::java_throwable::JavaThrowable {
    let reason = error.map_or_else(String::new, |error| {
        let text = error.raw_os_error().map_or_else(|| error.to_string(), |code| io::Error::from_raw_os_error(code).to_string());
        let text = text.strip_prefix("failed to lookup address information: ").unwrap_or(&text);
        let text = text.find(" (os error ").map_or(text, |end| &text[..end]);
        text.trim_end().trim_end_matches('.').to_string()
    });
    let message = if cfg!(windows) { format!("{reason} ({host})") } else { format!("{host}: {reason}") };
    crate::util::java_throwable::JavaThrowable::new(format!("java.net.UnknownHostException: {message}"), GET_BY_NAME_FRAMES.to_vec())
}

pub fn java_bind_exception(error: &io::Error) -> String {
    let reason = match error.kind() {
        io::ErrorKind::AddrInUse => "Address already in use",
        io::ErrorKind::AddrNotAvailable => "Cannot assign requested address",
        io::ErrorKind::PermissionDenied => "Permission denied",
        _ => return format!("java.net.SocketException: {error}"),
    };
    if cfg!(target_os = "linux") {
        format!("io.netty.channel.unix.Errors$NativeIoException: bind(..) failed: {reason}")
    } else {
        format!("java.net.BindException: {reason}: bind")
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn get_by_name_reads_literals_and_fails_like_java() {
        assert_eq!(super::get_by_name("127.0.0.1").unwrap(), std::net::IpAddr::from([127, 0, 0, 1]));
        assert_eq!(super::get_by_name("::1").unwrap(), std::net::IpAddr::from(std::net::Ipv6Addr::LOCALHOST));
        assert_eq!(super::get_by_name("[::1]").unwrap(), std::net::IpAddr::from(std::net::Ipv6Addr::LOCALHOST));
        let throwable = super::get_by_name("nonexistent.invalid").unwrap_err();
        assert!(throwable.description.starts_with("java.net.UnknownHostException: "), "{}", throwable.description);
        assert!(throwable.description.contains("nonexistent.invalid"), "{}", throwable.description);
        assert!(!throwable.description.contains("os error"), "{}", throwable.description);
        assert_eq!(throwable.stack_trace[8], "net.minecraft.server.dedicated.DedicatedServer.initServer(DedicatedServer.java:181)");
    }
}
