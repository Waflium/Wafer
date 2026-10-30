use super::timed_stat::TimedStat;
use crate::rfr::consumer::recording_file::RecordedEvent;

#[derive(Clone, Debug)]
pub struct StructureGenStat {
    pub duration: i64,
    pub chunk_pos: (i32, i32),
    pub structure_name: Option<String>,
    pub level: Option<String>,
    pub success: bool,
}

impl StructureGenStat {
    pub fn from(event: &RecordedEvent) -> StructureGenStat {
        StructureGenStat {
            duration: event.get_duration().as_nanos() as i64,
            chunk_pos: (event.get_int("chunkPosX"), event.get_int("chunkPosX")),
            structure_name: event.get_string("structure"),
            level: event.get_string("level"),
            success: event.get_boolean("success"),
        }
    }
}

impl TimedStat for StructureGenStat {
    fn duration(&self) -> i64 {
        self.duration
    }
}
