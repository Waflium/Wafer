use std::sync::Mutex;

use serde_json::Value;
use wafer_protocol::nbt::Nbt;

use super::ClimateSettings;
use super::biome_generation_settings::BiomeGenerationSettings;
use crate::world::attribute::EnvironmentAttributeMap;
use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::density_function::float;
use crate::world::level::levelgen::placement::placed_feature::placed_feature_index;

pub struct InlineBiome {
    pub tag: Nbt,
    pub(super) climate: ClimateSettings,
    pub attributes: EnvironmentAttributeMap,
    pub generation: BiomeGenerationSettings,
}

static INLINE_BIOMES: Mutex<Vec<(String, &'static InlineBiome)>> = Mutex::new(Vec::new());

fn create(tag: &Nbt) -> InlineBiome {
    let json: Value = crate::nbt::with_json_source(|| crate::util::codec::to_json(tag));
    let climate = ClimateSettings {
        has_precipitation: json["has_precipitation"].as_bool().unwrap_or(false),
        temperature: float(&json["temperature"]).unwrap_or(0.0),
        frozen: json.get("temperature_modifier").and_then(Value::as_str) == Some("frozen"),
    };
    let attributes = json.get("attributes").map_or_else(EnvironmentAttributeMap::default, EnvironmentAttributeMap::parse);
    let features = json["features"]
        .as_array()
        .map(|steps| steps.iter().map(|step| step.as_array().into_iter().flatten().filter_map(|feature| feature.as_str().and_then(placed_feature_index)).collect()).collect())
        .unwrap_or_default();
    InlineBiome { tag: tag.clone(), climate, attributes, generation: BiomeGenerationSettings::new(features) }
}

pub fn register(tag: &Nbt) -> Biome {
    let key = crate::nbt::to_snbt(tag);
    let mut biomes = INLINE_BIOMES.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
    let index = match biomes.iter().position(|(existing, _)| *existing == key) {
        Some(index) => index,
        None => {
            biomes.push((key, Box::leak(Box::new(create(tag)))));
            biomes.len() - 1
        }
    };
    Biome(Biome::INLINE | index as u16)
}

pub fn get(biome: Biome) -> &'static InlineBiome {
    INLINE_BIOMES.lock().unwrap_or_else(|poisoned| poisoned.into_inner())[(biome.0 & !Biome::INLINE) as usize].1
}

pub fn attribute_maps() -> Vec<EnvironmentAttributeMap> {
    INLINE_BIOMES.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).iter().map(|(_, biome)| biome.attributes.clone()).collect()
}

pub fn identity_hash(biome: Biome) -> i32 {
    crate::util::java_collections::identity_hash_code_of(&format!("inline_biome:{}", biome.0 & !Biome::INLINE))
}
