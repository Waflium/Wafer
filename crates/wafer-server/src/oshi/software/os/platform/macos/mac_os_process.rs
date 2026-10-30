use std::ffi::c_void;

use crate::oshi::software::os::OSProcess;
use crate::oshi::util::platform::macos::sysctl_util::sysctl_i64;

const PROC_PIDTASKALLINFO: i32 = 2;

fn ticks_per_ms() -> i64 {
    let ticks_per_second = sysctl_i64("hw.tbfrequency", 1_000_000_000);
    ticks_per_second / 1000
}

pub fn create(pid: i32) -> Option<OSProcess> {
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64);
    let mut task_all_info: libc::proc_taskallinfo = unsafe { std::mem::zeroed() };
    let size = std::mem::size_of::<libc::proc_taskallinfo>() as i32;
    let result = unsafe { libc::proc_pidinfo(pid, PROC_PIDTASKALLINFO, 0, (&mut task_all_info as *mut libc::proc_taskallinfo).cast::<c_void>(), size) };
    if result < 0 || task_all_info.ptinfo.pti_threadnum < 1 {
        return None;
    }
    let ticks_per_ms = ticks_per_ms();
    let start_time = task_all_info.pbsd.pbi_start_tvsec as i64 * 1000 + task_all_info.pbsd.pbi_start_tvusec as i64 / 1000;
    Some(OSProcess {
        virtual_size: task_all_info.ptinfo.pti_virtual_size as i64,
        resident_set_size: task_all_info.ptinfo.pti_resident_size as i64,
        kernel_time: task_all_info.ptinfo.pti_total_system as i64 / ticks_per_ms,
        user_time: task_all_info.ptinfo.pti_total_user as i64 / ticks_per_ms,
        up_time: now - start_time,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_current_process_can_be_read() {
        let process = create(std::process::id() as i32).unwrap();
        assert!(process.virtual_size > 0);
    }
}
