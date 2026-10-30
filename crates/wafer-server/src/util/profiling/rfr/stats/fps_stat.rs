use crate::rfr::consumer::recording_file::RecordedEvent;

#[derive(Clone, Copy, Debug)]
pub struct FpsStat {
    pub fps: i32,
}

impl FpsStat {
    pub fn from(event: &RecordedEvent, field: &str) -> FpsStat {
        FpsStat { fps: event.get_int(field) }
    }
}
