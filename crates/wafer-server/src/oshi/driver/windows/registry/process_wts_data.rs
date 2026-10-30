use std::collections::HashMap;

use windows_sys::Win32::Foundation::GetLastError;
use windows_sys::Win32::System::RemoteDesktop::{WTS_CURRENT_SERVER_HANDLE, WTS_PROCESS_INFO_EXW, WTSEnumerateProcessesExW, WTSFreeMemoryExW};

const WTS_ANY_SESSION: u32 = 0xFFFF_FFFE;
const WTS_TYPE_PROCESS_INFO_LEVEL_1: i32 = 1;

#[derive(Clone, Debug)]
pub struct WtsInfo {
    pub virtual_size: i64,
    pub kernel_time: i64,
    pub user_time: i64,
}

pub fn query_process_wts_map(pids: Option<&[i32]>) -> HashMap<i32, WtsInfo> {
    let mut wts_map = HashMap::new();
    let mut level = 1u32;
    let mut process_info: *mut u16 = std::ptr::null_mut();
    let mut count = 0u32;
    let enumerated = unsafe { WTSEnumerateProcessesExW(WTS_CURRENT_SERVER_HANDLE, &mut level, WTS_ANY_SESSION, &mut process_info, &mut count) };
    if enumerated == 0 {
        error!("Failed to enumerate Processes. Error code: {}", unsafe { GetLastError() });
        return wts_map;
    }
    let infos = unsafe { std::slice::from_raw_parts(process_info.cast::<WTS_PROCESS_INFO_EXW>(), count as usize) };
    for info in infos {
        let pid = info.ProcessId as i32;
        if pids.is_none_or(|pids| pids.contains(&pid)) {
            wts_map.insert(pid, WtsInfo { virtual_size: i64::from(info.PagefileUsage), kernel_time: info.KernelTime / 10000, user_time: info.UserTime / 10000 });
        }
    }
    if unsafe { WTSFreeMemoryExW(WTS_TYPE_PROCESS_INFO_LEVEL_1, process_info.cast(), count) } == 0 {
        warn!("Failed to Free Memory for Processes. Error code: {}", unsafe { GetLastError() });
    }
    wts_map
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_process_is_listed() {
        let pid = std::process::id() as i32;
        let map = query_process_wts_map(Some(&[pid]));
        let info = map.get(&pid).expect("current process");
        assert!(info.virtual_size > 0);
    }
}
