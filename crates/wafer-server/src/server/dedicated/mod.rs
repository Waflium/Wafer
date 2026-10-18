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

pub fn add_shutdown_hook(events: mpsc::UnboundedSender<Event>) {
    let _ = SHUTDOWN_HOOK.set((events.clone(), std::sync::Mutex::new(false), std::sync::Condvar::new()));
    #[cfg(windows)]
    unsafe {
        windows_sys::Win32::System::Console::SetConsoleCtrlHandler(Some(console_ctrl_handler), 1);
    }
    #[cfg(unix)]
    for kind in [tokio::signal::unix::SignalKind::terminate(), tokio::signal::unix::SignalKind::hangup()] {
        if let Ok(mut signal) = tokio::signal::unix::signal(kind) {
            let events = events.clone();
            tokio::spawn(async move {
                if signal.recv().await.is_some() {
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

fn halt_and_wait() {
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
