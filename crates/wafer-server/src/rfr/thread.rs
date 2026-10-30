use std::cell::Cell;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU64, Ordering};

#[derive(Clone, Debug, PartialEq)]
pub struct ThreadInfo {
    pub key: u64,
    pub os_name: String,
    pub os_thread_id: i64,
    pub java_name: String,
    pub java_thread_id: i64,
}

static NEXT_KEY: AtomicU64 = AtomicU64::new(1);
static THREADS: Mutex<Vec<ThreadInfo>> = Mutex::new(Vec::new());

thread_local! {
    static CURRENT: Cell<u64> = const { Cell::new(0) };
}

pub fn current() -> u64 {
    CURRENT.with(|current| {
        if current.get() == 0 {
            let name = std::thread::current().name().unwrap_or_default().to_string();
            current.set(register(os_thread_id(), &name, true));
        }
        current.get()
    })
}

pub fn register_other(os_thread_id: i64, name: &str) -> u64 {
    register(os_thread_id, name, false)
}

fn register(os_thread_id: i64, name: &str, rename: bool) -> u64 {
    let mut threads = THREADS.lock().unwrap();
    if let Some(thread) = threads.iter_mut().find(|thread| thread.os_thread_id == os_thread_id) {
        if rename {
            thread.java_name = name.to_string();
            thread.os_name = name.to_string();
        }
        return thread.key;
    }
    let key = NEXT_KEY.fetch_add(1, Ordering::Relaxed);
    threads.push(ThreadInfo { key, os_name: name.to_string(), os_thread_id, java_name: name.to_string(), java_thread_id: key as i64 });
    key
}

pub fn registered() -> Vec<ThreadInfo> {
    THREADS.lock().unwrap().clone()
}

#[cfg(windows)]
pub fn os_thread_id() -> i64 {
    unsafe { windows_sys::Win32::System::Threading::GetCurrentThreadId() as i64 }
}

#[cfg(target_os = "linux")]
pub fn os_thread_id() -> i64 {
    unsafe { libc::gettid() as i64 }
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn os_thread_id() -> i64 {
    0
}

#[cfg(windows)]
pub fn os_thread_name(os_thread_id: i64) -> String {
    use windows_sys::Win32::Foundation::{CloseHandle, LocalFree};
    use windows_sys::Win32::System::Threading::{GetThreadDescription, OpenThread, THREAD_QUERY_LIMITED_INFORMATION};
    let handle = unsafe { OpenThread(THREAD_QUERY_LIMITED_INFORMATION, 0, os_thread_id as u32) };
    if handle.is_null() {
        return String::new();
    }
    let mut description: windows_sys::core::PWSTR = std::ptr::null_mut();
    let mut name = String::new();
    if unsafe { GetThreadDescription(handle, &mut description) } >= 0 && !description.is_null() {
        let mut units = Vec::new();
        let mut i = 0;
        while unsafe { *description.add(i) } != 0 {
            units.push(unsafe { *description.add(i) });
            i += 1;
        }
        name = String::from_utf16_lossy(&units);
        unsafe { LocalFree(description as _) };
    }
    unsafe { CloseHandle(handle) };
    name
}

#[cfg(target_os = "linux")]
pub fn os_thread_name(os_thread_id: i64) -> String {
    std::fs::read_to_string(format!("/proc/self/task/{os_thread_id}/comm")).map(|name| name.trim_end_matches('\n').to_string()).unwrap_or_default()
}

#[cfg(not(any(windows, target_os = "linux")))]
pub fn os_thread_name(_os_thread_id: i64) -> String {
    String::new()
}
