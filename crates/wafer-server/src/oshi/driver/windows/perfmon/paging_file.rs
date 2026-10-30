use crate::oshi::util::platform::windows::perf_counter_query::{PdhCounterProperty, query_values};

const PAGING_PERCENT_PROPERTIES: [PdhCounterProperty; 1] = [PdhCounterProperty { instance: "_Total", counter: "% Usage", wmi_property: "PERCENTUSAGE" }];

pub fn query_swap_used() -> Vec<i64> {
    query_values(&PAGING_PERCENT_PROPERTIES, "Paging File", "Win32_PerfRawData_PerfOS_PagingFile")
}
