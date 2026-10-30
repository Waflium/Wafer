use super::timed_stat::TimedStat;
use crate::util::profiling::rfr::percentiles;

pub struct TimedStatSummary<T> {
    pub fastest: T,
    pub slowest: T,
    pub second_slowest: Option<T>,
    pub count: i32,
    pub percentiles_nanos: Vec<(i32, f64)>,
    pub total_duration: i64,
}

impl<T: TimedStat + Clone> TimedStatSummary<T> {
    pub fn summary(values: &[T]) -> Option<TimedStatSummary<T>> {
        if values.is_empty() {
            return None;
        }
        let mut sorted = values.to_vec();
        sorted.sort_by_key(TimedStat::duration);
        let total_duration = sorted.iter().map(TimedStat::duration).sum();
        let percentiles_nanos = percentiles::evaluate_longs(&sorted.iter().map(TimedStat::duration).collect::<Vec<_>>());
        Some(TimedStatSummary {
            fastest: sorted[0].clone(),
            slowest: sorted[sorted.len() - 1].clone(),
            second_slowest: (sorted.len() > 1).then(|| sorted[sorted.len() - 2].clone()),
            count: sorted.len() as i32,
            percentiles_nanos,
            total_duration,
        })
    }
}
