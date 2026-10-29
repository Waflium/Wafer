use serde_json::Value;

use crate::util::random::{WeightedList, weighted_codec, weighted_list_codec};
use crate::world::entity::mob_category::MobCategory;
use crate::world::item::component::types::{Pair, enumeration, record, req};
use crate::world::level::biome::mob_spawn_settings::{self, SpawnerData};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BoundingBoxType {
    Piece,
    Full,
}

#[derive(Clone, Debug)]
pub struct StructureSpawnOverride {
    pub bounding_box: BoundingBoxType,
    pub spawns: WeightedList<SpawnerData>,
}

impl BoundingBoxType {
    pub const NAMES: &'static [&'static str] = &["piece", "full"];
}

pub fn codec() -> Pair {
    record(vec![
        req("bounding_box", enumeration(BoundingBoxType::NAMES)),
        req("spawns", weighted_list_codec(weighted_codec(mob_spawn_settings::spawner_data_fields()))),
    ])
}

impl StructureSpawnOverride {
    fn parse(json: &Value) -> Result<StructureSpawnOverride, String> {
        let bounding_box = match json.get("bounding_box").and_then(Value::as_str) {
            Some("piece") => BoundingBoxType::Piece,
            Some("full") => BoundingBoxType::Full,
            other => return Err(format!("unknown bounding box type {other:?}")),
        };
        let spawns = mob_spawn_settings::parse_spawner_list(json.get("spawns").ok_or("missing spawns")?)?;
        Ok(StructureSpawnOverride { bounding_box, spawns })
    }

    pub fn parse_map(json: Option<&Value>) -> Result<[Option<StructureSpawnOverride>; 8], String> {
        let mut overrides: [Option<StructureSpawnOverride>; 8] = Default::default();
        let Some(map) = json.and_then(Value::as_object) else { return Ok(overrides) };
        for (name, entry) in map {
            let category = MobCategory::by_name(name).ok_or_else(|| format!("unknown mob category {name}"))?;
            overrides[category as usize] = Some(StructureSpawnOverride::parse(entry)?);
        }
        Ok(overrides)
    }
}
