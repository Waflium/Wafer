use serde_json::Value;

use super::biome_check::BiomeCheck;
use super::moon_brightness_check::MoonBrightnessCheck;
use super::spawn_context::SpawnContext;
use super::structure_check::StructureCheck;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::advancements::predicates::min_max_bounds::Doubles;
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::custom::DecodeOnly;
use crate::world::item::component::types::{self, Pair, req};

pub enum SpawnCondition {
    Biome(BiomeCheck),
    MoonBrightness(MoonBrightnessCheck),
    Structure(StructureCheck),
}

impl SpawnCondition {
    pub fn from_json(condition: &Value) -> SpawnCondition {
        match condition["type"].as_str() {
            Some("minecraft:moon_brightness") => SpawnCondition::MoonBrightness(MoonBrightnessCheck::from_json(condition)),
            Some("minecraft:structure") => SpawnCondition::Structure(StructureCheck::from_json(condition)),
            _ => SpawnCondition::Biome(BiomeCheck::from_json(condition)),
        }
    }

    pub fn test<L: ServerLevelAccessor + ?Sized>(&self, context: &mut SpawnContext<L>) -> bool {
        match self {
            SpawnCondition::Biome(check) => check.test(context.biome.0 as i32),
            SpawnCondition::MoonBrightness(check) => check.test(context),
            SpawnCondition::Structure(check) => check.test(context),
        }
    }
}

pub fn codec() -> Pair {
    let cases = vec![
        ("minecraft:structure", vec![req("structures", types::holder_set("minecraft:worldgen/structure"))]),
        ("minecraft:moon_brightness", vec![req("range", (Codec::Custom(Box::new(DecodeOnly(|tag| Doubles::decode(tag).map(drop)))), Stream::Custom))]),
        ("minecraft:biome", vec![req("biomes", types::holder_set("minecraft:worldgen/biome"))]),
    ];
    (Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:spawn_condition_type"), cases })), Stream::Dispatch)
}
