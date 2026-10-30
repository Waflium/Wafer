use crate::oshi::util::platform::windows::perf_counter_wildcard_query::{InstancesAndValues, WildcardProperties, query_instances_and_values};

pub const PROCESS_PERFORMANCE_COUNTERS: [&str; 9] = [
    "^*_Total",
    "Priority Base",
    "Elapsed Time",
    "ID Process",
    "Creating Process ID",
    "IO Read Bytes/sec",
    "IO Write Bytes/sec",
    "Working Set - Private",
    "Page Faults/sec",
];

const PROCESS_PERFORMANCE_WMI_PROPERTIES: [&str; 9] =
    ["NAME", "PRIORITYBASE", "ELAPSEDTIME", "IDPROCESS", "CREATINGPROCESSID", "IOREADBYTESPERSEC", "IOWRITEBYTESPERSEC", "PRIVATEBYTES", "PAGEFAULTSPERSEC"];

pub fn query_process_counters() -> InstancesAndValues {
    query_instances_and_values(
        &WildcardProperties { counters: &PROCESS_PERFORMANCE_COUNTERS, wmi_properties: &PROCESS_PERFORMANCE_WMI_PROPERTIES },
        "Process",
        "Win32_PerfRawData_PerfProc_Process WHERE NOT Name LIKE \"%_Total\"",
        None,
    )
}
