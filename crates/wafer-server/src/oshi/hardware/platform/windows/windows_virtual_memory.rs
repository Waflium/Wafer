use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::ProcessStatus::{GetPerformanceInfo, PERFORMANCE_INFORMATION};

use super::windows_global_memory::read_perf_info;
use crate::oshi::OshiResult;
use crate::oshi::driver::windows::perfmon::paging_file;
use crate::oshi::hardware::VirtualMemory;

pub struct WindowsVirtualMemory {
    page_size: i64,
}

impl WindowsVirtualMemory {
    pub fn new() -> WindowsVirtualMemory {
        WindowsVirtualMemory { page_size: read_perf_info().2 }
    }
}

fn query_swap_used() -> i64 {
    paging_file::query_swap_used().first().copied().unwrap_or(0)
}

fn query_swap_total_virt_max_virt_used() -> (i64, i64, i64) {
    let mut performance_info: PERFORMANCE_INFORMATION = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<PERFORMANCE_INFORMATION>() as u32;
    performance_info.cb = size;
    if unsafe { GetPerformanceInfo(&mut performance_info, size) } == 0 {
        error!("Failed to get Performance Info. Error code: {}", unsafe { GetLastError() });
        return (0, 0, 0);
    }
    (
        performance_info.CommitLimit as i64 - performance_info.PhysicalTotal as i64,
        performance_info.CommitLimit as i64,
        performance_info.CommitTotal as i64,
    )
}

impl VirtualMemory for WindowsVirtualMemory {
    fn get_virtual_max(&self) -> OshiResult<i64> {
        Ok(self.page_size * query_swap_total_virt_max_virt_used().1)
    }

    fn get_virtual_in_use(&self) -> OshiResult<i64> {
        Ok(self.page_size * query_swap_total_virt_max_virt_used().2)
    }

    fn get_swap_total(&self) -> OshiResult<i64> {
        Ok(self.page_size * query_swap_total_virt_max_virt_used().0)
    }

    fn get_swap_used(&self) -> OshiResult<i64> {
        Ok(self.page_size * query_swap_used())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn this_machine_reports_virtual_memory() {
        let virtual_memory = WindowsVirtualMemory::new();
        assert!(virtual_memory.get_virtual_max().ok().unwrap() > 0);
        assert!(virtual_memory.get_virtual_in_use().ok().unwrap() > 0);
        assert!(virtual_memory.get_swap_total().ok().unwrap() >= 0);
        assert!(virtual_memory.get_swap_used().ok().unwrap() >= 0);
    }
}
