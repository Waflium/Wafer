use crate::rfr::consumer::recording_file::RecordedEvent;

#[derive(Clone, Copy, Debug)]
pub struct CpuLoadStat {
    pub rust: f64,
    pub user_rust: f64,
    pub system: f64,
}

impl CpuLoadStat {
    pub fn from(event: &RecordedEvent) -> CpuLoadStat {
        CpuLoadStat { rust: event.get_float("rustSystem") as f64, user_rust: event.get_float("rustUser") as f64, system: event.get_float("machineTotal") as f64 }
    }
}
