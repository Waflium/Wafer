use std::collections::HashMap;
use std::sync::{Mutex, OnceLock, PoisonError};

use windows_sys::Win32::System::Performance::{PERF_COUNTER_BLOCK, PERF_COUNTER_DEFINITION, PERF_DATA_BLOCK, PERF_INSTANCE_DEFINITION, PERF_OBJECT_TYPE};
use windows_sys::Win32::System::Registry::{HKEY_PERFORMANCE_DATA, RegQueryValueExW};

use crate::jna::platform::win32::advapi32_util::{ROOT_LOCAL_MACHINE, RegistryError, registry_get_string_array};
use crate::jna::platform::win32::{from_wide_nul, wide};
use crate::util::java_stack;

const HKEY_PERFORMANCE_TEXT: &str = "SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion\\Perflib\\009";
const COUNTER: &str = "Counter";
const ERROR_MORE_DATA: u32 = 234;

const MAP_COUNTER_INDICES_FRAME: &str = "oshi.driver.windows.registry.HkeyPerformanceDataUtil.mapCounterIndicesFromRegistry(HkeyPerformanceDataUtil.java:274)";
const CLINIT_FRAME: &str = "oshi.driver.windows.registry.HkeyPerformanceDataUtil.<clinit>(HkeyPerformanceDataUtil.java:51)";

static MAX_PERF_BUFFER_SIZE: Mutex<usize> = Mutex::new(16384);

#[derive(Clone, Debug, PartialEq)]
pub enum PerfValue {
    Int(i32),
    Long(i64),
    Name(String),
}

impl PerfValue {
    pub fn as_int(&self) -> i32 {
        match self {
            PerfValue::Int(value) => *value,
            PerfValue::Long(value) => *value as i32,
            PerfValue::Name(_) => 0,
        }
    }

    pub fn as_long(&self) -> i64 {
        match self {
            PerfValue::Int(value) => i64::from(*value),
            PerfValue::Long(value) => *value,
            PerfValue::Name(_) => 0,
        }
    }

    pub fn as_name(&self) -> &str {
        match self {
            PerfValue::Name(name) => name,
            _ => "",
        }
    }
}

pub struct PerfData {
    pub instances: Vec<Vec<PerfValue>>,
    pub now: i64,
}

pub fn counter_index_map() -> &'static HashMap<String, i32> {
    static COUNTER_INDEX_MAP: OnceLock<HashMap<String, i32>> = OnceLock::new();
    COUNTER_INDEX_MAP.get_or_init(|| {
        let _scope = java_stack::enter_all(&[MAP_COUNTER_INDICES_FRAME, CLINIT_FRAME]);
        map_counter_indices_from_registry()
    })
}

fn map_counter_indices_from_registry() -> HashMap<String, i32> {
    let mut index_map = HashMap::new();
    match registry_get_string_array(ROOT_LOCAL_MACHINE, HKEY_PERFORMANCE_TEXT, COUNTER) {
        Ok(counter_text) => {
            let mut i = 1;
            while i < counter_text.len() {
                match counter_text[i - 1].parse::<i32>() {
                    Ok(index) => {
                        index_map.entry(counter_text[i].clone()).or_insert(index);
                    }
                    Err(_) => {
                        error!("Unable to parse English counter names in registry Perflib 009.");
                        break;
                    }
                }
                i += 2;
            }
        }
        Err(RegistryError::Win32(exception)) => {
            error!("Unable to locate English counter names in registry Perflib 009. Counters may need to be rebuilt: \n{}", exception.throwable.print_stack_trace().trim_end());
        }
        Err(RegistryError::Other(_)) => {}
    }
    index_map
}

fn counter_indices(object_name: &str, counter_names: &[&str]) -> Option<(i32, Vec<i32>)> {
    let map = counter_index_map();
    let counter_index = *map.get(object_name)?;
    let mut indices = Vec::with_capacity(counter_names.len());
    for counter_name in counter_names {
        indices.push(*map.get(*counter_name)?);
    }
    Some((counter_index, indices))
}

fn read_perf_data_buffer(object_name: &str) -> Option<Vec<u8>> {
    let object_index = counter_index_map().get(object_name)?.to_string();
    let name = wide(&object_index);
    let mut max_size = MAX_PERF_BUFFER_SIZE.lock().unwrap_or_else(PoisonError::into_inner);
    loop {
        let mut data = vec![0u8; *max_size];
        let mut size = *max_size as u32;
        let result = unsafe { RegQueryValueExW(HKEY_PERFORMANCE_DATA, name.as_ptr(), std::ptr::null(), std::ptr::null_mut(), data.as_mut_ptr(), &mut size) };
        if result == 0 {
            return Some(data);
        }
        if result != ERROR_MORE_DATA {
            error!("Error reading performance data from registry for {object_name}.");
            return None;
        }
        *max_size += 8192;
    }
}

fn read<T: Copy>(data: &[u8], offset: usize) -> Option<T> {
    (offset.checked_add(std::mem::size_of::<T>())? <= data.len()).then(|| unsafe { std::ptr::read_unaligned(data.as_ptr().add(offset).cast::<T>()) })
}

fn read_wide_string(data: &[u8], offset: usize) -> String {
    let units: Vec<u16> = data.get(offset..).unwrap_or_default().chunks_exact(2).map(|pair| u16::from_le_bytes([pair[0], pair[1]])).collect();
    from_wide_nul(&units)
}

pub fn filetime_to_millis(filetime: i64) -> i64 {
    filetime / 10000 - 11_644_473_600_000
}

pub fn read_perf_data_from_registry(object_name: &str, counter_names: &[&str]) -> Option<PerfData> {
    let (object_index, counter_name_indices) = counter_indices(object_name, counter_names)?;
    let data = read_perf_data_buffer(object_name)?;
    let perf_data: PERF_DATA_BLOCK = read(&data, 0)?;
    let perf_time_100n_sec = perf_data.PerfTime100nSec;
    let now = filetime_to_millis(perf_time_100n_sec);
    let mut perf_object_offset = perf_data.HeaderLength as usize;
    for _ in 0..perf_data.NumObjectTypes {
        let perf_object: PERF_OBJECT_TYPE = read(&data, perf_object_offset)?;
        if perf_object.ObjectNameTitleIndex as i32 == object_index {
            let mut perf_counter_offset = perf_object_offset + perf_object.HeaderLength as usize;
            let mut counter_offset_map = HashMap::new();
            let mut counter_size_map = HashMap::new();
            for _ in 0..perf_object.NumCounters {
                let perf_counter: PERF_COUNTER_DEFINITION = read(&data, perf_counter_offset)?;
                counter_offset_map.insert(perf_counter.CounterNameTitleIndex as i32, perf_counter.CounterOffset as usize);
                counter_size_map.insert(perf_counter.CounterNameTitleIndex as i32, perf_counter.CounterSize);
                perf_counter_offset += perf_counter.ByteLength as usize;
            }
            let mut perf_instance_offset = perf_object_offset + perf_object.DefinitionLength as usize;
            let mut counter_maps = Vec::with_capacity(perf_object.NumInstances.max(0) as usize);
            for _ in 0..perf_object.NumInstances {
                let perf_instance: PERF_INSTANCE_DEFINITION = read(&data, perf_instance_offset)?;
                let perf_counter_block_offset = perf_instance_offset + perf_instance.ByteLength as usize;
                let mut counter_map = Vec::with_capacity(counter_names.len() + 1);
                counter_map.push(PerfValue::Name(read_wide_string(&data, perf_instance_offset + perf_instance.NameOffset as usize)));
                for key_index in &counter_name_indices {
                    let size = counter_size_map.get(key_index).copied().unwrap_or(0);
                    let offset = perf_counter_block_offset + *counter_offset_map.get(key_index)?;
                    match size {
                        4 => counter_map.push(PerfValue::Int(read::<i32>(&data, offset)?)),
                        8 => counter_map.push(PerfValue::Long(read::<i64>(&data, offset)?)),
                        _ => return None,
                    }
                }
                counter_maps.push(counter_map);
                let counter_block: PERF_COUNTER_BLOCK = read(&data, perf_counter_block_offset)?;
                perf_instance_offset = perf_counter_block_offset + counter_block.ByteLength as usize;
            }
            return Some(PerfData { instances: counter_maps, now });
        }
        perf_object_offset += perf_object.TotalByteLength as usize;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn filetimes_convert_to_epoch_milliseconds() {
        assert_eq!(filetime_to_millis(116_444_736_000_000_000), 0);
    }

    #[test]
    fn values_convert_between_widths() {
        assert_eq!(PerfValue::Int(-1).as_long(), -1);
        assert_eq!(PerfValue::Long(5).as_int(), 5);
        assert_eq!(PerfValue::Name("java".to_string()).as_name(), "java");
    }
}
