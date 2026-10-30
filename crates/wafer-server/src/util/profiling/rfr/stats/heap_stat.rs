use super::{millis, seconds};
use crate::rfr::consumer::recording_file::RecordedEvent;

#[derive(Clone, Copy, Debug)]
pub struct HeapStat {
    pub timestamp: i64,
    pub heap_used: i64,
    pub allocated: i64,
}

pub struct Summary {
    pub duration: i64,
    pub gc_total_duration: i64,
    pub total_gcs: i32,
    pub allocation_rate_bytes_per_second: f64,
}

impl Summary {
    pub fn gc_over_head(&self) -> f32 {
        millis(self.gc_total_duration) as f32 / millis(self.duration) as f32
    }
}

impl HeapStat {
    pub fn from(event: &RecordedEvent) -> HeapStat {
        HeapStat { timestamp: event.get_start_time(), heap_used: event.get_long("heapUsed"), allocated: event.get_long("allocated") }
    }

    pub fn summary(recording_duration: i64, heap_stats: &[HeapStat], gc_total_duration: i64, total_gcs: i32) -> Result<Summary, String> {
        Ok(Summary { duration: recording_duration, gc_total_duration, total_gcs, allocation_rate_bytes_per_second: calculate_allocation_rate_per_second(heap_stats)? })
    }
}

fn calculate_allocation_rate_per_second(heap_stats: &[HeapStat]) -> Result<f64, String> {
    if heap_stats.len() < 2 {
        return Err(format!("index out of bounds: the len is {} but the index is 1", heap_stats.len()));
    }
    let first = heap_stats[0];
    let last = heap_stats[heap_stats.len() - 1];
    Ok((last.allocated - first.allocated) as f64 / seconds(last.timestamp - first.timestamp) as f64)
}
