use std::sync::OnceLock;

const MAX_ARRAY_LENGTH: i64 = 2_147_483_645;
const ARRAY_HEADER_BYTES: i64 = 16;
const FALLBACK_MAX_HEAP_BYTES: u64 = 8 << 30;

#[cfg(all(target_os = "linux", not(test)))]
fn physical_memory() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kilobytes: u64 = meminfo.lines().find_map(|line| line.strip_prefix("MemTotal:"))?.trim().strip_suffix("kB")?.trim().parse().ok()?;
    Some(kilobytes * 1024)
}

#[cfg(all(windows, not(test)))]
fn physical_memory() -> Option<u64> {
    use windows_sys::Win32::System::SystemInformation::{GlobalMemoryStatusEx, MEMORYSTATUSEX};
    let mut status = MEMORYSTATUSEX { dwLength: std::mem::size_of::<MEMORYSTATUSEX>() as u32, ..unsafe { std::mem::zeroed() } };
    (unsafe { GlobalMemoryStatusEx(&mut status) } != 0).then_some(status.ullTotalPhys)
}

#[cfg(all(target_os = "macos", not(test)))]
fn physical_memory() -> Option<u64> {
    let mut memory: u64 = 0;
    let mut size = std::mem::size_of::<u64>();
    let found = unsafe { libc::sysctlbyname(c"hw.memsize".as_ptr(), (&mut memory as *mut u64).cast(), &mut size, std::ptr::null_mut(), 0) } == 0;
    found.then_some(memory)
}

#[cfg(any(test, not(any(target_os = "linux", target_os = "macos", windows))))]
fn physical_memory() -> Option<u64> {
    None
}

pub fn max_heap_bytes() -> u64 {
    static MAX_HEAP: OnceLock<u64> = OnceLock::new();
    *MAX_HEAP.get_or_init(|| physical_memory().map_or(FALLBACK_MAX_HEAP_BYTES, |physical| physical / 4))
}

pub fn array_allocation_failure(length: i64, element_size: i64, live_bytes: u64) -> Option<String> {
    if length > MAX_ARRAY_LENGTH {
        return Some("java.lang.OutOfMemoryError: Requested array size exceeds VM limit".to_string());
    }
    if (length * element_size + ARRAY_HEADER_BYTES) as u64 + live_bytes > max_heap_bytes() {
        return Some("java.lang.OutOfMemoryError: Java heap space".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_arrays_fail_like_the_jvm() {
        assert_eq!(array_allocation_failure(i32::MAX as i64, 1, 0).as_deref(), Some("java.lang.OutOfMemoryError: Requested array size exceeds VM limit"));
        assert_eq!(array_allocation_failure(2_147_483_646, 8, 0).as_deref(), Some("java.lang.OutOfMemoryError: Requested array size exceeds VM limit"));
        assert_eq!(array_allocation_failure(2_147_483_645, 8, 0).as_deref(), Some("java.lang.OutOfMemoryError: Java heap space"));
        assert_eq!(array_allocation_failure(16, 8, 0), None);
        assert_eq!(array_allocation_failure(1 << 30, 4, 5 << 30).as_deref(), Some("java.lang.OutOfMemoryError: Java heap space"));
    }
}
