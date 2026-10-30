use super::timed_stat::TimedStat;
use crate::rfr::consumer::recording_file::RecordedEvent;

#[derive(Clone, Debug)]
pub struct ChunkGenStat {
    pub duration: i64,
    pub chunk_pos: (i32, i32),
    pub world_pos: (i32, i32),
    pub status: &'static str,
    pub level: Option<String>,
}

impl ChunkGenStat {
    pub fn from(event: &RecordedEvent) -> ChunkGenStat {
        ChunkGenStat {
            duration: event.get_duration().as_nanos() as i64,
            chunk_pos: (event.get_int("chunkPosX"), event.get_int("chunkPosX")),
            world_pos: (event.get_int("worldPosX"), event.get_int("worldPosZ")),
            status: by_name(event.get_string("status").as_deref()),
            level: event.get_string("level"),
        }
    }
}

impl TimedStat for ChunkGenStat {
    fn duration(&self) -> i64 {
        self.duration
    }
}

const STATUSES: [&str; 10] = [
    "minecraft:empty",
    "minecraft:structure_starts",
    "minecraft:structure_references",
    "minecraft:biomes",
    "minecraft:terrain",
    "minecraft:features",
    "minecraft:initialize_light",
    "minecraft:light",
    "minecraft:spawn",
    "minecraft:full",
];

fn by_name(key: Option<&str>) -> &'static str {
    let id = key.and_then(|key| crate::resources::identifier::parse(key).ok());
    STATUSES.iter().find(|status| Some(**status) == id.as_deref()).copied().unwrap_or(STATUSES[0])
}
