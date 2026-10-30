use crate::rfr::consumer::recording_file::RecordedEvent;
use crate::util::java_collections::java_string_hash_code;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ChunkIdentification {
    pub level: Option<String>,
    pub dimension: Option<String>,
    pub x: i32,
    pub z: i32,
}

impl ChunkIdentification {
    pub fn from(event: &RecordedEvent) -> ChunkIdentification {
        ChunkIdentification { level: event.get_string("level"), dimension: event.get_string("dimension"), x: event.get_int("chunkPosX"), z: event.get_int("chunkPosZ") }
    }

    pub fn hash_code(&self) -> i32 {
        let mut result = 0i32;
        for hash in [java_string_hash_code(self.level.as_deref()), java_string_hash_code(self.dimension.as_deref()), self.x, self.z] {
            result = result.wrapping_mul(31).wrapping_add(hash);
        }
        result
    }
}
