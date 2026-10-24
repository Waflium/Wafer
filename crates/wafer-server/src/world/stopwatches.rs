use std::io;
use std::path::PathBuf;
use std::sync::LazyLock;
use std::time::Instant;

use wafer_core::fastutil::Object2ObjectOpenHashMap;
use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::java_collections::identifier_hash;
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::level::storage::{read_saved_data, write_dat};

#[derive(Clone, Copy, Debug)]
pub struct Stopwatch {
    creation_time: i64,
    accumulated_elapsed_time: i64,
}

impl Stopwatch {
    pub fn new(creation_time: i64) -> Stopwatch {
        Stopwatch { creation_time, accumulated_elapsed_time: 0 }
    }

    pub fn elapsed_milliseconds(&self, current_time: i64) -> i64 {
        self.accumulated_elapsed_time + (current_time - self.creation_time)
    }

    pub fn elapsed_seconds(&self, current_time: i64) -> f64 {
        self.elapsed_milliseconds(current_time) as f64 / 1000.0
    }
}

pub struct Stopwatches {
    dir: Option<PathBuf>,
    stopwatches: Object2ObjectOpenHashMap<Stopwatch>,
    dirty: bool,
}

impl Stopwatches {
    fn file(dir: &std::path::Path) -> PathBuf {
        dir.join("minecraft").join("stopwatches.dat")
    }

    pub fn current_time() -> i64 {
        static START: LazyLock<Instant> = LazyLock::new(Instant::now);
        START.elapsed().as_millis() as i64
    }

    pub fn load(dir: Option<PathBuf>) -> Stopwatches {
        let entries = dir.as_deref().and_then(|dir| read_saved_data(&Stopwatches::file(dir), "Server thread", "stopwatches", DataFixTypes::SavedDataStopwatches, Stopwatches::decode));
        let mut stopwatches = Object2ObjectOpenHashMap::with_hash(16, 0.75, identifier_hash);
        let dirty = entries.is_none();
        let current_time = Stopwatches::current_time();
        for (id, elapsed) in entries.into_iter().flatten() {
            stopwatches.insert(id, Stopwatch { creation_time: current_time, accumulated_elapsed_time: elapsed });
        }
        Stopwatches { dir, stopwatches, dirty }
    }

    fn decode(data: &Nbt) -> DecodeResult<Vec<(String, i64)>> {
        if let Err(error) = codec::map(data) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let entries = record.required(data, "stopwatches", |entries| codec::base_map_partial(entries, crate::resources::identifier::read, |elapsed| codec::long(elapsed).into()));
        record.build(|| entries)
    }

    pub fn ids(&self) -> impl Iterator<Item = &str> {
        self.stopwatches.iter().map(|(id, _)| id)
    }

    pub fn get(&self, id: &str) -> Option<&Stopwatch> {
        self.stopwatches.get(id)
    }

    pub fn add(&mut self, id: &str, stopwatch: Stopwatch) -> bool {
        if self.stopwatches.contains_key(id) {
            return false;
        }
        self.stopwatches.insert(id.to_string(), stopwatch);
        self.dirty = true;
        true
    }

    pub fn update(&mut self, id: &str, update: impl FnOnce(Stopwatch) -> Stopwatch) -> bool {
        let Some(stopwatch) = self.stopwatches.get_mut(id) else { return false };
        *stopwatch = update(*stopwatch);
        self.dirty = true;
        true
    }

    pub fn remove(&mut self, id: &str) -> bool {
        let removed = self.stopwatches.remove(id).is_some();
        self.dirty |= removed;
        removed
    }

    fn is_dirty(&self) -> bool {
        self.dirty || self.stopwatches.len() > 0
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.is_dirty() {
            return Ok(());
        }
        let current_time = Stopwatches::current_time();
        let entries = self.stopwatches.iter().map(|(id, s)| (id.to_string(), Nbt::Long(s.elapsed_milliseconds(current_time)))).collect();
        let data = Nbt::Compound(vec![("stopwatches".into(), Nbt::Compound(entries))]);
        write_dat(&Stopwatches::file(dir), &Nbt::Compound(vec![("data".into(), data), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]))?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_errors_match_vanilla() {
        let cases: [(&str, Option<&str>, Option<&[&str]>); 7] = [
            ("{}", Some("No key stopwatches in MapLike[{}]"), None),
            ("{stopwatches:5}", Some("Not a map: 5"), None),
            ("{stopwatches:{}}", None, Some(&[])),
            (r#"{stopwatches:{"a":1L,"b":"x"}}"#, Some(r#"Not a number missed input: {b:"x"}"#), Some(&["minecraft:a"])),
            (
                r#"{stopwatches:{"a":1L,"B":2L}}"#,
                Some("Not a valid resource location: B Non [a-z0-9/._-] character in path of location: minecraft:B missed input: {B:2L}"),
                Some(&["minecraft:a"]),
            ),
            (r#"{stopwatches:{"a:b":1L,"c":2}}"#, None, Some(&["a:b", "minecraft:c"])),
            ("{stopwatches:[]}", Some("Not a map: []"), None),
        ];
        for (input, error, ids) in cases {
            let tag = crate::nbt::parse_fully(input).ok().unwrap();
            let result = codec::with_nbt_ops(|| Stopwatches::decode(&tag));
            assert_eq!(result.error.as_deref(), error, "{input}");
            let decoded: Option<Vec<String>> = result.value.map(|entries| {
                let mut ids: Vec<String> = entries.into_iter().map(|(id, _)| id).collect();
                ids.sort();
                ids
            });
            assert_eq!(decoded, ids.map(|ids| ids.iter().map(|id| id.to_string()).collect()), "{input}");
        }
    }
}
