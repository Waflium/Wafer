use super::seconds;
use crate::util::java_collections::{java_hash_map_iteration_order, java_string_hash_code};

#[derive(Clone, Debug)]
pub struct FileIOStat {
    pub duration: i64,
    pub path: Option<String>,
    pub bytes: i64,
}

pub struct Summary {
    pub total_bytes: i64,
    pub bytes_per_second: f64,
    pub counts: i64,
    pub counts_per_second: f64,
    pub time_spent_in_io: i64,
    pub top_ten_contributors_by_total_bytes: Vec<(String, i64)>,
}

impl FileIOStat {
    pub fn summary(recording_duration: i64, io_stats: &[FileIOStat]) -> Summary {
        let total_bytes: i64 = io_stats.iter().map(|stat| stat.bytes).sum();
        let mut by_path: Vec<(String, i64)> = Vec::new();
        for stat in io_stats {
            let Some(path) = &stat.path else { continue };
            match by_path.iter_mut().find(|(existing, _)| existing == path) {
                Some((_, bytes)) => *bytes += stat.bytes,
                None => by_path.push((path.clone(), stat.bytes)),
            }
        }
        let hashes: Vec<i32> = by_path.iter().map(|(path, _)| java_string_hash_code(Some(path))).collect();
        let mut contributors: Vec<(String, i64)> = java_hash_map_iteration_order(&hashes).into_iter().map(|i| by_path[i].clone()).collect();
        contributors.sort_by(|(_, a), (_, b)| b.cmp(a));
        contributors.truncate(10);
        Summary {
            total_bytes,
            bytes_per_second: total_bytes as f64 / seconds(recording_duration) as f64,
            counts: io_stats.len() as i64,
            counts_per_second: io_stats.len() as f64 / seconds(recording_duration) as f64,
            time_spent_in_io: io_stats.iter().map(|stat| stat.duration).sum(),
            top_ten_contributors_by_total_bytes: contributors,
        }
    }
}
