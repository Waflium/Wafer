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

#[cfg(all(windows, not(test)))]
fn resident_set_size() -> Option<u64> {
    use windows_sys::Win32::System::ProcessStatus::{GetProcessMemoryInfo, PROCESS_MEMORY_COUNTERS};
    use windows_sys::Win32::System::Threading::GetCurrentProcess;
    let mut counters: PROCESS_MEMORY_COUNTERS = unsafe { std::mem::zeroed() };
    counters.cb = std::mem::size_of::<PROCESS_MEMORY_COUNTERS>() as u32;
    (unsafe { GetProcessMemoryInfo(GetCurrentProcess(), &mut counters, counters.cb) } != 0).then_some(counters.WorkingSetSize as u64)
}

#[cfg(all(target_os = "linux", not(test)))]
fn resident_set_size() -> Option<u64> {
    let statm = std::fs::read_to_string("/proc/self/statm").ok()?;
    let pages: u64 = statm.split_whitespace().nth(1)?.parse().ok()?;
    Some(pages * unsafe { libc::sysconf(libc::_SC_PAGESIZE) } as u64)
}

#[cfg(any(test, not(any(target_os = "linux", windows))))]
fn resident_set_size() -> Option<u64> {
    None
}

const G1_TARGET_REGION_COUNT: u64 = 2048;
const G1_MIN_REGION_SIZE: u64 = 1 << 20;
const G1_MAX_REGION_SIZE: u64 = 32 << 20;
const INITIAL_RAM_FRACTION: u64 = 64;
const MAX_RAM_FRACTION: u64 = 4;
const EMULATED_NON_HEAP_USED: u64 = 128 << 20;
const EMULATED_NON_HEAP_COMMITTED: u64 = 136 << 20;
const EMULATED_NON_HEAP_INIT: u64 = 7_667_712;
const EMULATED_MIN_HEAP_USED: u64 = 32 << 20;

fn g1_region_size(initial_heap: u64, max_heap: u64) -> u64 {
    let average_heap = (initial_heap + max_heap) / 2;
    let region_size = (average_heap / G1_TARGET_REGION_COUNT).max(G1_MIN_REGION_SIZE);
    (1 << region_size.ilog2()).clamp(G1_MIN_REGION_SIZE, G1_MAX_REGION_SIZE)
}

fn align_up(value: u64, alignment: u64) -> u64 {
    value.div_ceil(alignment) * alignment
}

pub fn heap_sizes_for(physical: Option<u64>) -> (u64, u64, u64) {
    let (initial, max) = match physical {
        Some(physical) => (physical / INITIAL_RAM_FRACTION, physical / MAX_RAM_FRACTION),
        None => (FALLBACK_MAX_HEAP_BYTES / 16, FALLBACK_MAX_HEAP_BYTES),
    };
    let region_size = g1_region_size(initial, max);
    (align_up(initial, region_size), align_up(max, region_size), region_size)
}

pub fn max_heap_bytes() -> u64 {
    static MAX_HEAP: OnceLock<u64> = OnceLock::new();
    *MAX_HEAP.get_or_init(|| heap_sizes_for(physical_memory()).1)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MemoryUsage {
    pub init: i64,
    pub used: i64,
    pub committed: i64,
    pub max: i64,
}

pub fn heap_memory_usage_for(physical: Option<u64>, resident_set_size: Option<u64>) -> MemoryUsage {
    let (initial, max, region_size) = heap_sizes_for(physical);
    let used = resident_set_size.map_or(EMULATED_MIN_HEAP_USED, |resident| resident.saturating_sub(EMULATED_NON_HEAP_USED).max(EMULATED_MIN_HEAP_USED)).min(max);
    let committed = initial.max(align_up(used, region_size)).min(max);
    MemoryUsage { init: initial as i64, used: used as i64, committed: committed as i64, max: max as i64 }
}

pub fn heap_memory_usage() -> MemoryUsage {
    heap_memory_usage_for(physical_memory(), resident_set_size())
}

pub fn non_heap_memory_usage() -> MemoryUsage {
    MemoryUsage { init: EMULATED_NON_HEAP_INIT as i64, used: EMULATED_NON_HEAP_USED as i64, committed: EMULATED_NON_HEAP_COMMITTED as i64, max: -1 }
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
    fn default_heap_sizes_follow_the_g1_ergonomics() {
        let physical = 32512u64 << 20;
        let (initial, max, region_size) = heap_sizes_for(Some(physical));
        assert_eq!((initial >> 20, max >> 20, region_size >> 20), (508, 8128, 2));
        let (_, max, region_size) = heap_sizes_for(Some((16 << 30) - 123));
        assert_eq!((max >> 20, region_size >> 20), (4096, 1));
    }

    #[test]
    fn emulated_heap_usage_is_consistent() {
        let usage = heap_memory_usage_for(Some(32512u64 << 20), Some(600 << 20));
        assert!(usage.init <= usage.committed && usage.used <= usage.committed && usage.committed <= usage.max);
        assert_eq!(usage.committed, usage.init);
        let grown = heap_memory_usage_for(Some(32512u64 << 20), Some(2000 << 20));
        assert_eq!(grown.used, (2000 << 20) - (128 << 20));
        assert!(grown.committed >= grown.used && grown.committed % (2 << 20) == 0);
        assert_eq!(non_heap_memory_usage().max, -1);
    }

    #[test]
    fn huge_arrays_fail_like_the_jvm() {
        assert_eq!(array_allocation_failure(i32::MAX as i64, 1, 0).as_deref(), Some("java.lang.OutOfMemoryError: Requested array size exceeds VM limit"));
        assert_eq!(array_allocation_failure(2_147_483_646, 8, 0).as_deref(), Some("java.lang.OutOfMemoryError: Requested array size exceeds VM limit"));
        assert_eq!(array_allocation_failure(2_147_483_645, 8, 0).as_deref(), Some("java.lang.OutOfMemoryError: Java heap space"));
        assert_eq!(array_allocation_failure(16, 8, 0), None);
        assert_eq!(array_allocation_failure(1 << 30, 4, 5 << 30).as_deref(), Some("java.lang.OutOfMemoryError: Java heap space"));
    }
}
