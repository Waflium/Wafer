use windows_sys::Win32::System::Performance::{PdhEnumObjectItemsW, PdhLookupPerfNameByIndexW};

use super::advapi32_util::{ROOT_LOCAL_MACHINE, RegistryError, registry_get_string_array};
use super::{from_wide_nul, wide};
use crate::util::java_stack;
use crate::util::java_throwable::JavaThrowable;

const ENGLISH_COUNTER_KEY: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Perflib\\009";
const ENGLISH_COUNTER_VALUE: &str = "Counter";

const PDH_MORE_DATA: u32 = 0x800007D2;
const PDH_INVALID_ARGUMENT: u32 = 0xC0000BBD;
const PDH_INSUFFICIENT_BUFFER: u32 = 0xC0000BC2;

pub struct PdhException {
    pub throwable: JavaThrowable,
}

impl PdhException {
    fn new(error_code: u32) -> PdhException {
        PdhException {
            throwable: JavaThrowable::new(
                format!("com.sun.jna.platform.win32.PdhUtil$PdhException: Pdh call failed with error code 0x{error_code:08X}"),
                java_stack::capture(&[]),
            ),
        }
    }
}

pub struct PdhEnumObjectItems {
    pub instances: Vec<String>,
}

pub fn pdh_lookup_perf_name_by_index(index: i32) -> Result<String, PdhException> {
    let mut size = 0u32;
    let result = unsafe { PdhLookupPerfNameByIndexW(std::ptr::null(), index as u32, std::ptr::null_mut(), &mut size) };
    let mut buffer: Vec<u16>;
    let result = if result != PDH_INVALID_ARGUMENT {
        if result != 0 && result != PDH_MORE_DATA {
            return Err(PdhException::new(result));
        }
        if (size as i32) < 1 {
            return Ok(String::new());
        }
        buffer = vec![0u16; size as usize];
        unsafe { PdhLookupPerfNameByIndexW(std::ptr::null(), index as u32, buffer.as_mut_ptr(), &mut size) }
    } else {
        let mut buffer_size = 32usize;
        buffer = Vec::new();
        let mut result = result;
        while buffer_size <= 1024 {
            size = buffer_size as u32;
            buffer = vec![0u16; buffer_size];
            result = unsafe { PdhLookupPerfNameByIndexW(std::ptr::null(), index as u32, buffer.as_mut_ptr(), &mut size) };
            if result != PDH_INVALID_ARGUMENT && result != PDH_INSUFFICIENT_BUFFER {
                break;
            }
            buffer_size *= 2;
        }
        result
    };
    if result != 0 {
        return Err(PdhException::new(result));
    }
    Ok(from_wide_nul(&buffer))
}

pub fn pdh_lookup_perf_index_by_english_name(name: &str) -> Result<i32, RegistryError> {
    let counters = registry_get_string_array(ROOT_LOCAL_MACHINE, ENGLISH_COUNTER_KEY, ENGLISH_COUNTER_VALUE)?;
    let mut i = 1;
    while i < counters.len() {
        if counters[i] == name {
            return Ok(counters[i - 1].parse().unwrap_or(0));
        }
        i += 2;
    }
    Ok(0)
}

fn multi_strings(buffer: &[u16]) -> Vec<String> {
    let mut strings = Vec::new();
    let mut offset = 0;
    while offset < buffer.len() {
        let end = buffer[offset..].iter().position(|&unit| unit == 0).map_or(buffer.len(), |position| offset + position);
        if end == offset {
            break;
        }
        strings.push(String::from_utf16_lossy(&buffer[offset..end]));
        offset = end + 1;
    }
    strings
}

pub fn pdh_enum_object_items(object_name: &str, detail_level: u32) -> Result<PdhEnumObjectItems, PdhException> {
    let object = wide(object_name);
    let (mut counter_length, mut instance_length) = (0u32, 0u32);
    let result = unsafe {
        PdhEnumObjectItemsW(std::ptr::null(), std::ptr::null(), object.as_ptr(), std::ptr::null_mut(), &mut counter_length, std::ptr::null_mut(), &mut instance_length, detail_level, 0)
    };
    if result != 0 && result != PDH_MORE_DATA {
        return Err(PdhException::new(result));
    }
    loop {
        let mut counters = vec![0u16; counter_length as usize];
        let mut instances = vec![0u16; instance_length as usize];
        let result = unsafe {
            PdhEnumObjectItemsW(
                std::ptr::null(),
                std::ptr::null(),
                object.as_ptr(),
                if counters.is_empty() { std::ptr::null_mut() } else { counters.as_mut_ptr() },
                &mut counter_length,
                if instances.is_empty() { std::ptr::null_mut() } else { instances.as_mut_ptr() },
                &mut instance_length,
                detail_level,
                0,
            )
        };
        if result == PDH_MORE_DATA {
            counter_length = counters.len() as u32 + 1024;
            instance_length = instances.len() as u32 + 1024;
            continue;
        }
        if result != 0 {
            return Err(PdhException::new(result));
        }
        return Ok(PdhEnumObjectItems { instances: multi_strings(&instances) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn multi_strings_stop_at_the_empty_string() {
        let units: Vec<u16> = "a\0b#1\0\0".encode_utf16().collect();
        assert_eq!(multi_strings(&units), vec!["a", "b#1"]);
    }

    #[test]
    fn process_instances_can_be_enumerated() {
        let items = pdh_enum_object_items("Process", 100).ok().unwrap();
        assert!(items.instances.iter().any(|instance| instance == "_Total"));
    }

    #[test]
    fn counter_names_are_localized_by_index() {
        assert!(!pdh_lookup_perf_name_by_index(230).ok().unwrap().is_empty());
    }
}
