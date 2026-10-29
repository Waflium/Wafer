use std::sync::OnceLock;

const MAX_ARRAY_LENGTH: i64 = 2_147_483_645;
const ARRAY_HEADER_BYTES: i64 = 16;
const FALLBACK_MAX_HEAP_BYTES: u64 = 4 << 30;

#[cfg(target_os = "linux")]
fn physical_memory() -> Option<u64> {
    let meminfo = std::fs::read_to_string("/proc/meminfo").ok()?;
    let kilobytes: u64 = meminfo.lines().find_map(|line| line.strip_prefix("MemTotal:"))?.trim().strip_suffix("kB")?.trim().parse().ok()?;
    Some(kilobytes * 1024)
}

#[cfg(not(target_os = "linux"))]
fn physical_memory() -> Option<u64> {
    None
}

pub fn max_heap_bytes() -> u64 {
    static MAX_HEAP: OnceLock<u64> = OnceLock::new();
    *MAX_HEAP.get_or_init(|| physical_memory().map_or(FALLBACK_MAX_HEAP_BYTES, |physical| physical / 4))
}

pub fn array_allocation_failure(length: i64, element_size: i64) -> Option<String> {
    if length > MAX_ARRAY_LENGTH {
        return Some("java.lang.OutOfMemoryError: Requested array size exceeds VM limit".to_string());
    }
    if (length * element_size + ARRAY_HEADER_BYTES) as u64 > max_heap_bytes() {
        return Some("java.lang.OutOfMemoryError: Java heap space".to_string());
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn huge_arrays_fail_like_the_jvm() {
        assert_eq!(array_allocation_failure(i32::MAX as i64, 1).as_deref(), Some("java.lang.OutOfMemoryError: Requested array size exceeds VM limit"));
        assert_eq!(array_allocation_failure(2_147_483_646, 8).as_deref(), Some("java.lang.OutOfMemoryError: Requested array size exceeds VM limit"));
        assert_eq!(array_allocation_failure(2_147_483_645, 8).as_deref(), Some("java.lang.OutOfMemoryError: Java heap space"));
        assert_eq!(array_allocation_failure(16, 8), None);
    }
}
