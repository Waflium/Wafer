use super::mac_global_memory::{query_available, query_phys_mem};
use crate::oshi::OshiResult;
use crate::oshi::hardware::VirtualMemory;
use crate::oshi::util::platform::macos::sysctl_util::sysctl_struct;

pub struct MacVirtualMemory;

fn query_swap_usage() -> (i64, i64) {
    match sysctl_struct("vm.swapusage", 32) {
        Some(bytes) => {
            let total = u64::from_ne_bytes(bytes[0..8].try_into().unwrap_or([0; 8]));
            let used = u64::from_ne_bytes(bytes[16..24].try_into().unwrap_or([0; 8]));
            (used as i64, total as i64)
        }
        None => (0, 0),
    }
}

impl VirtualMemory for MacVirtualMemory {
    fn get_virtual_max(&self) -> OshiResult<i64> {
        Ok(query_phys_mem() + query_swap_usage().1)
    }

    fn get_virtual_in_use(&self) -> OshiResult<i64> {
        Ok(query_phys_mem() - query_available() + query_swap_usage().0)
    }

    fn get_swap_total(&self) -> OshiResult<i64> {
        Ok(query_swap_usage().1)
    }

    fn get_swap_used(&self) -> OshiResult<i64> {
        Ok(query_swap_usage().0)
    }
}
