use std::collections::HashMap;

use windows_sys::Win32::Foundation::{CloseHandle, GetLastError};
use windows_sys::Win32::System::Threading::{OpenProcess, QueryFullProcessImageNameW};

use crate::oshi::driver::windows::registry::process_performance_data::PerfCounterBlock;
use crate::oshi::driver::windows::registry::process_wts_data::WtsInfo;
use crate::oshi::software::os::OSProcess;

const PROCESS_QUERY_INFORMATION: u32 = 1024;
const ERROR_INSUFFICIENT_BUFFER: u32 = 122;

fn query_full_process_image_name(process: windows_sys::Win32::Foundation::HANDLE) -> Result<String, u32> {
    let mut size = 260u32;
    loop {
        let mut buffer = vec![0u16; size as usize];
        let mut length = size;
        if unsafe { QueryFullProcessImageNameW(process, 0, buffer.as_mut_ptr(), &mut length) } != 0 {
            return Ok(String::from_utf16_lossy(&buffer[..length as usize]));
        }
        let error = unsafe { GetLastError() };
        if error != ERROR_INSUFFICIENT_BUFFER || size >= 32768 {
            return Err(error);
        }
        size *= 2;
    }
}

pub fn create(pid: i32, process_map: &HashMap<i32, PerfCounterBlock>, process_wts_map: &HashMap<i32, WtsInfo>) -> Option<OSProcess> {
    let (pcb, wts) = (process_map.get(&pid)?, process_wts_map.get(&pid)?);
    let process = OSProcess {
        virtual_size: wts.virtual_size,
        resident_set_size: pcb.resident_set_size,
        kernel_time: wts.kernel_time,
        user_time: wts.user_time,
        up_time: pcb.up_time,
    };
    let handle = unsafe { OpenProcess(PROCESS_QUERY_INFORMATION, 0, pid as u32) };
    if !handle.is_null() {
        let valid = query_full_process_image_name(handle).is_ok();
        unsafe { CloseHandle(handle) };
        if !valid {
            return None;
        }
    }
    Some(process)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn processes_missing_from_either_map_are_not_created() {
        assert!(create(1, &HashMap::new(), &HashMap::new()).is_none());
    }
}
