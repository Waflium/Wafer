use super::seconds;
use crate::util::java_collections::{java_hash_map_iteration_order, java_string_hash_code};
use wafer_core::java_string::JavaStr;
use crate::rfr::consumer::recording_file::RecordedEvent;

#[derive(Clone, Debug)]
pub struct ThreadAllocationStat {
    pub timestamp: i64,
    pub thread_name: String,
    pub total_bytes: i64,
}

pub struct Summary {
    pub allocations_per_second_by_thread: Vec<(String, f64)>,
}

impl ThreadAllocationStat {
    pub fn from(event: &RecordedEvent) -> ThreadAllocationStat {
        let thread_name = event.get_thread("thread").and_then(|thread| thread.get_java_name().map(str::to_string)).unwrap_or_else(|| "unknown".to_string());
        ThreadAllocationStat { timestamp: event.get_start_time(), thread_name, total_bytes: event.get_long("allocated") }
    }

    pub fn summary(stats: &[ThreadAllocationStat]) -> Summary {
        let mut by_thread: Vec<(String, Vec<&ThreadAllocationStat>)> = Vec::new();
        for stat in stats {
            match by_thread.iter_mut().find(|(thread, _)| *thread == stat.thread_name) {
                Some((_, thread_stats)) => thread_stats.push(stat),
                None => by_thread.push((stat.thread_name.clone(), vec![stat])),
            }
        }
        let mut allocations_per_second_by_thread = Vec::new();
        let hashes: Vec<i32> = by_thread.iter().map(|(thread, _)| java_string_hash_code(Some(thread))).collect();
        for index in java_hash_map_iteration_order(&hashes) {
            let (thread, thread_stats) = &by_thread[index];
            if thread_stats.len() >= 2 {
                let first = thread_stats[0];
                let last = thread_stats[thread_stats.len() - 1];
                let duration = seconds(last.timestamp - first.timestamp);
                if duration <= 0 {
                    warn!("Thread allocation stat timestamps are not in chronological order for thread {thread}, skipping it");
                } else {
                    allocations_per_second_by_thread.push((thread.clone(), (last.total_bytes - first.total_bytes) as f64 / duration as f64));
                }
            }
        }
        allocations_per_second_by_thread.sort_by(|(a, _), (b, _)| a.java_utf16().cmp(b.java_utf16()));
        Summary { allocations_per_second_by_thread }
    }
}
