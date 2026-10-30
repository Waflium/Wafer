use std::collections::HashMap;

use super::perf_data_util::{self, PerfCounter};

#[derive(Default)]
pub struct PerfCounterQueryHandler {
    counter_handle_map: HashMap<u64, usize>,
    query_handle: Option<usize>,
}

impl PerfCounterQueryHandler {
    pub fn new() -> PerfCounterQueryHandler {
        PerfCounterQueryHandler::default()
    }

    pub fn add_counter_to_query(&mut self, counter: &PerfCounter) -> bool {
        let query = match self.query_handle {
            Some(query) => query,
            None => match perf_data_util::open_query() {
                Some(query) => {
                    self.query_handle = Some(query);
                    query
                }
                None => {
                    warn!("Failed to open a query for PDH counter: {}", counter.get_counter_path());
                    return false;
                }
            },
        };
        match perf_data_util::add_counter(query, &counter.get_counter_path()) {
            Some(handle) => {
                self.counter_handle_map.insert(counter.id, handle);
                true
            }
            None => {
                warn!("Failed to add counter for PDH counter: {}", counter.get_counter_path());
                false
            }
        }
    }

    pub fn remove_all_counters(&mut self) {
        for (_, handle) in self.counter_handle_map.drain() {
            perf_data_util::remove_counter(handle);
        }
        if let Some(query) = self.query_handle.take() {
            perf_data_util::close_query(query);
        }
    }

    pub fn update_query(&mut self) -> i64 {
        match self.query_handle {
            Some(query) => perf_data_util::update_query_timestamp(query),
            None => {
                warn!("Query does not exist to update.");
                0
            }
        }
    }

    pub fn query_counter(&self, counter: &PerfCounter) -> i64 {
        let Some(&handle) = self.counter_handle_map.get(&counter.id) else {
            warn!("Counter {} does not exist to query.", counter.get_counter_path());
            return 0;
        };
        let value = if counter.is_base_counter() { perf_data_util::query_second_counter(handle) } else { perf_data_util::query_counter(handle) };
        if value < 0 {
            warn!("Error querying counter {}: {}", counter.get_counter_path(), perf_data_util::format_error(value as i32 as u32));
            return 0;
        }
        value
    }
}

impl Drop for PerfCounterQueryHandler {
    fn drop(&mut self) {
        self.remove_all_counters();
    }
}
