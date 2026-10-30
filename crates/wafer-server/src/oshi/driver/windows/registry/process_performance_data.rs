use std::collections::HashMap;

use super::hkey_performance_data_util::{filetime_to_millis, read_perf_data_from_registry};
use crate::oshi::OshiResult;
use crate::oshi::driver::windows::perfmon::process_information::{PROCESS_PERFORMANCE_COUNTERS, query_process_counters};
use crate::util::java_stack;
use crate::util::java_throwable::JavaThrowable;

const BUILD_PROCESS_MAP_FROM_REGISTRY_FRAME: &str = "oshi.driver.windows.registry.ProcessPerformanceData.buildProcessMapFromRegistry(ProcessPerformanceData.java:45)";
const BUILD_PROCESS_MAP_FROM_PERF_COUNTERS_FRAME: &str = "oshi.driver.windows.registry.ProcessPerformanceData.buildProcessMapFromPerfCounters(ProcessPerformanceData.java:80)";
const PROCESS: &str = "Process";
const ELAPSED_TIME: usize = 1;
const ID_PROCESS: usize = 2;
const PRIVATE_BYTES: usize = 6;

#[derive(Clone, Debug)]
pub struct PerfCounterBlock {
    pub resident_set_size: i64,
    pub up_time: i64,
}

fn counter_block(now: i64, elapsed_time: i64, private_bytes: i64) -> PerfCounterBlock {
    let mut start_time = elapsed_time;
    if start_time > now {
        start_time = filetime_to_millis(start_time);
    }
    PerfCounterBlock { resident_set_size: private_bytes, up_time: (now - start_time).max(1) }
}

pub fn build_process_map_from_registry(pids: Option<&[i32]>) -> Option<HashMap<i32, PerfCounterBlock>> {
    let _scope = java_stack::enter(BUILD_PROCESS_MAP_FROM_REGISTRY_FRAME);
    let process_data = read_perf_data_from_registry(PROCESS, &PROCESS_PERFORMANCE_COUNTERS[1..])?;
    let now = process_data.now;
    let mut process_map = HashMap::new();
    for instance in &process_data.instances {
        let name = instance[0].as_name();
        let pid = instance[1 + ID_PROCESS].as_int();
        if pids.is_none_or(|pids| pids.contains(&pid)) && name != "_Total" {
            process_map.insert(pid, counter_block(now, instance[1 + ELAPSED_TIME].as_long(), instance[1 + PRIVATE_BYTES].as_long()));
        }
    }
    Some(process_map)
}

pub fn build_process_map_from_perf_counters(pids: Option<&[i32]>) -> OshiResult<HashMap<i32, PerfCounterBlock>> {
    let _scope = java_stack::enter(BUILD_PROCESS_MAP_FROM_PERF_COUNTERS_FRAME);
    let instance_values = query_process_counters();
    let now = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |elapsed| elapsed.as_millis() as i64);
    let mut process_map = HashMap::new();
    for index in 0..instance_values.instances.len() {
        let Some(pid_list) = instance_values.values.get(ID_PROCESS) else {
            return Err(JavaThrowable::new(
                "java.lang.NullPointerException: Cannot invoke \"java.util.List.get(int)\" because \"pidList\" is null",
                java_stack::capture(&[]),
            ));
        };
        let pid = pid_list[index] as i32;
        if pids.is_none_or(|pids| pids.contains(&pid)) {
            process_map.insert(pid, counter_block(now, instance_values.values[ELAPSED_TIME][index], instance_values.values[PRIVATE_BYTES][index]));
        }
    }
    Ok(process_map)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_times_convert_from_filetimes_and_uptime_is_at_least_one_millisecond() {
        let now = 1_700_000_000_000;
        let filetime = (now - 5000 + 11_644_473_600_000) * 10000;
        let block = counter_block(now, filetime, 3);
        assert_eq!((block.up_time, block.resident_set_size), (5000, 3));
        assert_eq!(counter_block(now, now, 3).up_time, 1);
    }
}
