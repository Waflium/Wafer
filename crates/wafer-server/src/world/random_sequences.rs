use std::io;
use std::path::PathBuf;

use wafer_core::fastutil::Object2ObjectOpenHashMap;
use wafer_core::random::{XoroshiroRandomSource, mix_stafford13, seed_from_hash_of};
use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult, Record};
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::extra_codecs;
use crate::util::java_collections::identifier_hash;
use crate::world::level::storage::{read_saved_data_dat, write_dat};

fn create_sequence(seed: i64, key: Option<&str>) -> XoroshiroRandomSource {
    let mut lo = seed ^ 7640891576956012809;
    let mut hi = lo.wrapping_add(-7046029254386353131);
    if let Some(key) = key {
        let (key_lo, key_hi) = seed_from_hash_of(key);
        lo ^= key_lo;
        hi ^= key_hi;
    }
    XoroshiroRandomSource::from_parts(mix_stafford13(lo), mix_stafford13(hi))
}

pub struct RandomSequences {
    dir: Option<PathBuf>,
    salt: i32,
    include_world_seed: bool,
    include_sequence_id: bool,
    sequences: Object2ObjectOpenHashMap<XoroshiroRandomSource>,
    dirty: bool,
}

impl RandomSequences {
    fn empty(dir: Option<PathBuf>) -> RandomSequences {
        RandomSequences {
            dir,
            salt: 0,
            include_world_seed: true,
            include_sequence_id: true,
            sequences: Object2ObjectOpenHashMap::with_hash(16, 0.75, identifier_hash),
            dirty: false,
        }
    }

    fn file(dir: &std::path::Path) -> PathBuf {
        dir.join("minecraft").join("random_sequences.dat")
    }

    fn decode_sequence(tag: &Nbt) -> DecodeResult<XoroshiroRandomSource> {
        match codec::field(tag, "source") {
            Ok(source) => extra_codecs::fixed_size_longs(source, 2).map(|state| XoroshiroRandomSource::from_parts(state[0], state[1])),
            Err(error) => DecodeResult::error(error),
        }
    }

    fn decode(dir: Option<PathBuf>, data: &Nbt) -> DecodeResult<RandomSequences> {
        if let Err(error) = codec::map(data) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let salt = record.required(data, "salt", codec::int);
        let include_world_seed = record.with_default(data, "include_world_seed", true, codec::bool_value);
        let include_sequence_id = record.with_default(data, "include_sequence_id", true, codec::bool_value);
        let entries = record.required(data, "sequences", |tag| codec::base_map_partial(tag, crate::resources::identifier::read, RandomSequences::decode_sequence));
        record.build(|| {
            let mut sequences = RandomSequences::empty(dir);
            sequences.salt = salt?;
            sequences.include_world_seed = include_world_seed;
            sequences.include_sequence_id = include_sequence_id;
            for (key, sequence) in entries? {
                sequences.sequences.insert(key, sequence);
            }
            Some(sequences)
        })
    }

    pub fn load(dir: Option<PathBuf>) -> RandomSequences {
        let parsed = dir.as_deref().and_then(|dir| match read_saved_data_dat(&RandomSequences::file(dir), DataFixTypes::SavedDataRandomSequences) {
            Ok(Some(tag)) => {
                let decoded = codec::with_nbt_ops(|| match tag.get("data") {
                    Some(data) => RandomSequences::decode(Some(dir.to_path_buf()), data),
                    None => DecodeResult::error("Not a map: null"),
                });
                if let Some(error) = decoded.error {
                    error!(thread: "main", "Failed to parse saved data for 'SavedDataType[minecraft:random_sequences]': {error}");
                }
                decoded.value
            }
            Ok(None) => None,
            Err(_) => {
                error!(thread: "main", "Error loading saved data: SavedDataType[minecraft:random_sequences]");
                None
            }
        });
        parsed.unwrap_or_else(|| {
            let mut sequences = RandomSequences::empty(dir);
            sequences.dirty = true;
            sequences
        })
    }

    fn create_sequence(&self, key: &str, world_seed: i64, salt: i32, include_world_seed: bool, include_sequence_id: bool) -> XoroshiroRandomSource {
        let seed = (if include_world_seed { world_seed } else { 0 }) ^ salt as i64;
        create_sequence(seed, include_sequence_id.then_some(key))
    }

    pub fn get(&mut self, key: &str, world_seed: i64) -> &mut XoroshiroRandomSource {
        if !self.sequences.contains_key(key) {
            let sequence = self.create_sequence(key, world_seed, self.salt, self.include_world_seed, self.include_sequence_id);
            self.sequences.insert(key.to_string(), sequence);
        }
        self.dirty = true;
        self.sequences.get_mut(key).unwrap()
    }

    pub fn keys(&self) -> impl Iterator<Item = &str> {
        self.sequences.iter().map(|(k, _)| k)
    }

    pub fn set_seed_defaults(&mut self, salt: i32, include_world_seed: bool, include_sequence_id: bool) {
        self.salt = salt;
        self.include_world_seed = include_world_seed;
        self.include_sequence_id = include_sequence_id;
    }

    pub fn clear(&mut self) -> i32 {
        let count = self.sequences.len() as i32;
        self.sequences.clear();
        count
    }

    pub fn reset(&mut self, id: &str, world_seed: i64) {
        let sequence = self.create_sequence(id, world_seed, self.salt, self.include_world_seed, self.include_sequence_id);
        self.sequences.insert(id.to_string(), sequence);
    }

    pub fn reset_with(&mut self, id: &str, world_seed: i64, salt: i32, include_world_seed: bool, include_sequence_id: bool) {
        let sequence = self.create_sequence(id, world_seed, salt, include_world_seed, include_sequence_id);
        self.sequences.insert(id.to_string(), sequence);
    }

    fn to_nbt(&self) -> Nbt {
        let sequences = self
            .sequences
            .iter()
            .map(|(key, source)| {
                let (lo, hi) = source.state();
                (key.to_string(), Nbt::Compound(vec![("source".into(), Nbt::LongArray(vec![lo, hi]))]))
            })
            .collect();
        let mut fields = vec![("salt".into(), Nbt::Int(self.salt))];
        if !self.include_world_seed {
            fields.push(("include_world_seed".into(), Nbt::Byte(0)));
        }
        if !self.include_sequence_id {
            fields.push(("include_sequence_id".into(), Nbt::Byte(0)));
        }
        fields.push(("sequences".into(), Nbt::Compound(sequences)));
        Nbt::Compound(fields)
    }

    pub fn save(&mut self) -> io::Result<()> {
        let Some(dir) = &self.dir else { return Ok(()) };
        if !self.dirty {
            return Ok(());
        }
        let file = Nbt::Compound(vec![("data".into(), self.to_nbt()), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        write_dat(&RandomSequences::file(dir), &file)?;
        self.dirty = false;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::random::RandomSource;

    use super::*;

    #[test]
    fn sequences_match_vanilla() {
        let mut a = create_sequence(42, Some("minecraft:a"));
        let mut b = create_sequence(42 ^ 7, None);
        let a: Vec<i32> = (0..3).map(|_| wafer_core::mth::random_between_inclusive(&mut a, 1, 100)).collect();
        let b: Vec<i32> = (0..3).map(|_| wafer_core::mth::random_between_inclusive(&mut b, -5, 5)).collect();
        assert_eq!((a, b), (vec![49, 82, 30], vec![4, -4, 0]));
    }

    #[test]
    fn decode_errors_match_vanilla() {
        let cases: [(&str, Option<&str>, Option<&[&str]>); 19] = [
            ("5", Some("Not a map: 5"), None),
            ("{}", Some("No key sequences in MapLike[{}]; No key salt in MapLike[{}]"), None),
            ("{salt:1}", Some("No key sequences in MapLike[{salt:1}]"), None),
            ("{sequences:{}}", Some("No key salt in MapLike[{sequences:{}}]"), None),
            ("{salt:\"x\",sequences:{}}", Some("Not a number"), None),
            ("{salt:1,sequences:5}", Some("Not a map: 5"), None),
            ("{salt:1,sequences:[]}", Some("Not a map: []"), None),
            ("{salt:1,sequences:{\"minecraft:a\":5}}", Some("Not a map: 5 missed input: {\"minecraft:a\":5}"), Some(&[])),
            ("{salt:1,sequences:{\"minecraft:a\":{}}}", Some("No key source in MapLike[{}] missed input: {\"minecraft:a\":{}}"), Some(&[])),
            ("{salt:1,sequences:{\"minecraft:a\":{source:5}}}", Some("Not a list missed input: {\"minecraft:a\":{source:5}}"), Some(&[])),
            ("{salt:1,sequences:{\"minecraft:a\":{source:[L;1L]}}}", Some("Input is not a list of 2 longs missed input: {\"minecraft:a\":{source:[L;1L]}}"), Some(&[])),
            (
                "{salt:1,sequences:{\"minecraft:a\":{source:[L;1L,2L,3L]}}}",
                Some("Input is not a list of 2 longs missed input: {\"minecraft:a\":{source:[L;1L,2L,3L]}}"),
                Some(&["minecraft:a"]),
            ),
            (
                "{salt:1,sequences:{\"minecraft:a\":{source:[\"x\",\"y\"]}}}",
                Some("Some elements are not longs: [\"x\",\"y\"] missed input: {\"minecraft:a\":{source:[\"x\",\"y\"]}}"),
                Some(&[]),
            ),
            ("{salt:1,sequences:{\"minecraft:a\":{source:[1,2]}}}", None, Some(&["minecraft:a"])),
            ("{salt:1,sequences:{\"minecraft:a\":{source:[I;1,2]}}}", None, Some(&["minecraft:a"])),
            ("{salt:1,include_world_seed:\"x\",sequences:{}}", Some("Not a number"), Some(&[])),
            ("{salt:1,include_world_seed:2,sequences:{}}", None, Some(&[])),
            ("{salt:\"x\",include_world_seed:\"y\",sequences:5}", Some("Not a map: 5; Not a number; Not a number"), None),
            (
                "{salt:1,sequences:{\"minecraft:a\":{source:[L;1L,2L]},\"minecraft:b\":{source:5},\"minecraft:c\":{}}}",
                Some("Not a list; No key source in MapLike[{}] missed input: {\"minecraft:b\":{source:5},\"minecraft:c\":{}}"),
                Some(&["minecraft:a"]),
            ),
        ];
        for (input, error, keys) in cases {
            let tag = crate::nbt::parse_fully(input).ok().unwrap();
            let result = codec::with_nbt_ops(|| RandomSequences::decode(None, &tag));
            assert_eq!(result.error.as_deref(), error, "{input}");
            let decoded: Option<Vec<String>> = result.value.map(|sequences| sequences.keys().map(str::to_string).collect());
            let expected: Option<Vec<String>> = keys.map(|keys| keys.iter().map(|key| key.to_string()).collect());
            assert_eq!(decoded, expected, "{input}");
        }
    }

    #[test]
    fn sequences_carry_on_after_a_reload() {
        let dir = std::env::temp_dir().join(format!("wafer-random-sequences-{}", std::process::id()));
        let mut sequences = RandomSequences::load(Some(dir.clone()));
        let first = sequences.get("minecraft:a", 42).next_int();
        sequences.save().unwrap();
        let expected = sequences.get("minecraft:a", 42).next_int();
        let mut loaded = RandomSequences::load(Some(dir.clone()));
        assert_eq!(loaded.get("minecraft:a", 42).next_int(), expected);
        assert_ne!(first, expected);
        std::fs::remove_dir_all(dir).unwrap();
    }
}
