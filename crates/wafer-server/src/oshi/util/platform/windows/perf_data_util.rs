use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};

use windows_sys::Win32::System::Performance::{
    PDH_RAW_COUNTER, PdhAddEnglishCounterW, PdhCloseQuery, PdhCollectQueryDataWithTime, PdhGetRawCounterValue, PdhOpenQueryW, PdhRemoveCounter,
};

use crate::jna::platform::win32::wide;

const PDH_NO_DATA: u32 = 0x800007D5;

pub fn format_error(error_code: u32) -> String {
    format!("0x{error_code:08X}")
}

static NEXT_COUNTER_ID: AtomicU64 = AtomicU64::new(0);

pub struct PerfCounter {
    pub id: u64,
    object: String,
    instance: Option<String>,
    counter: String,
    base_counter: bool,
}

impl PerfCounter {
    pub fn new(object_name: &str, instance_name: Option<&str>, counter_name: &str) -> PerfCounter {
        let (counter, base_counter) = match counter_name.find("_Base") {
            Some(index) if index > 0 => (counter_name[..index].to_string(), true),
            _ => (counter_name.to_string(), false),
        };
        PerfCounter {
            id: NEXT_COUNTER_ID.fetch_add(1, Ordering::Relaxed),
            object: object_name.to_string(),
            instance: instance_name.map(ToString::to_string),
            counter,
            base_counter,
        }
    }

    pub fn is_base_counter(&self) -> bool {
        self.base_counter
    }

    pub fn get_counter_path(&self) -> String {
        let mut path = format!("\\{}", self.object);
        if let Some(instance) = &self.instance {
            path.push('(');
            path.push_str(instance);
            path.push(')');
        }
        path.push('\\');
        path.push_str(&self.counter);
        path
    }
}

pub fn create_counter(object: &str, instance: Option<&str>, counter: &str) -> PerfCounter {
    PerfCounter::new(object, instance, counter)
}

pub fn filetime_to_utc_ms(filetime: i64, local: bool) -> i64 {
    let tz_offset = if local { i64::from(chrono::Local::now().offset().local_minus_utc()) * 1000 } else { 0 };
    filetime / 10000 - 11_644_473_600_000 - tz_offset
}

pub fn update_query_timestamp(query: usize) -> i64 {
    let mut timestamp = 0i64;
    let mut result = unsafe { PdhCollectQueryDataWithTime(query as *mut c_void, &mut timestamp) };
    let mut retries = 0;
    while result == PDH_NO_DATA && retries < 3 {
        retries += 1;
        std::thread::sleep(std::time::Duration::from_millis(1 << retries));
        result = unsafe { PdhCollectQueryDataWithTime(query as *mut c_void, &mut timestamp) };
    }
    if result != 0 {
        warn!("Failed to update counter. Error code: {}", format_error(result));
        return 0;
    }
    filetime_to_utc_ms(timestamp, true)
}

pub fn open_query() -> Option<usize> {
    let mut query: *mut c_void = std::ptr::null_mut();
    let result = unsafe { PdhOpenQueryW(std::ptr::null(), 0, &mut query) };
    if result != 0 {
        error!("Failed to open PDH Query. Error code: {}", format_error(result));
        return None;
    }
    Some(query as usize)
}

pub fn close_query(query: usize) -> bool {
    unsafe { PdhCloseQuery(query as *mut c_void) == 0 }
}

fn raw_counter(counter: usize) -> Result<PDH_RAW_COUNTER, u32> {
    let mut value: PDH_RAW_COUNTER = unsafe { std::mem::zeroed() };
    let mut counter_type = 16u32;
    let result = unsafe { PdhGetRawCounterValue(counter as *mut c_void, &mut counter_type, &mut value) };
    if result != 0 {
        warn!("Failed to get counter. Error code: {}", format_error(result));
        return Err(result);
    }
    Ok(value)
}

pub fn query_counter(counter: usize) -> i64 {
    match raw_counter(counter) {
        Ok(value) => value.FirstValue,
        Err(result) => i64::from(result as i32),
    }
}

pub fn query_second_counter(counter: usize) -> i64 {
    match raw_counter(counter) {
        Ok(value) => value.SecondValue,
        Err(result) => i64::from(result as i32),
    }
}

pub fn add_counter(query: usize, path: &str) -> Option<usize> {
    let mut counter: *mut c_void = std::ptr::null_mut();
    let result = unsafe { PdhAddEnglishCounterW(query as *mut c_void, wide(path).as_ptr(), 0, &mut counter) };
    if result != 0 {
        warn!("Failed to add PDH Counter: {path}, Error code: {}", format_error(result));
        return None;
    }
    Some(counter as usize)
}

pub fn remove_counter(counter: usize) -> bool {
    unsafe { PdhRemoveCounter(counter as *mut c_void) == 0 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn counter_paths_name_object_instance_and_counter() {
        assert_eq!(PerfCounter::new("Process", Some("java#1"), "ID Process").get_counter_path(), "\\Process(java#1)\\ID Process");
        assert_eq!(PerfCounter::new("Paging File", Some("_Total"), "% Usage").get_counter_path(), "\\Paging File(_Total)\\% Usage");
        assert_eq!(PerfCounter::new("Memory", None, "Pages Input/sec").get_counter_path(), "\\Memory\\Pages Input/sec");
        let base = PerfCounter::new("Processor", Some("0"), "% Idle Time_Base");
        assert!(base.is_base_counter());
        assert_eq!(base.get_counter_path(), "\\Processor(0)\\% Idle Time");
    }

    #[test]
    fn filetimes_convert_to_unix_milliseconds() {
        assert_eq!(filetime_to_utc_ms(116_444_736_000_000_000, false), 0);
        assert_eq!(filetime_to_utc_ms(116_444_736_000_000_000 + 10_000_000, false), 1000);
    }
}
