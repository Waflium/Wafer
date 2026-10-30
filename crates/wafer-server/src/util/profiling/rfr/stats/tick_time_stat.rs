use crate::rfr::consumer::recording_file::RecordedEvent;

#[derive(Clone, Copy, Debug)]
pub struct TickTimeStat {
    pub timestamp: i64,
    pub current_average: i64,
}

impl TickTimeStat {
    pub fn from(event: &RecordedEvent, ticks_per_second: i64) -> TickTimeStat {
        TickTimeStat { timestamp: event.get_start_time(), current_average: event.get_duration_field("averageTickDuration", ticks_per_second).as_nanos() as i64 }
    }
}
