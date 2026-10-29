pub mod biome_generation_settings;
pub mod biome_source;
pub mod biome_special_effects;
pub mod climate;
pub mod feature_sorter;
pub mod mob_spawn_settings;
pub mod the_end_biome_source;

use std::sync::LazyLock;

use wafer_core::random::LegacyRandomSource;
use wafer_core::BlockPos;
use serde_json::Value;
use wafer_protocol::nbt::Nbt;

use wafer_data::{Fluid, blocks};

use crate::world::attribute::EnvironmentAttributeMap;
use crate::world::item::component::codec::Codec;
use crate::world::item::component::json_to_nbt;
use crate::world::item::component::types::{self, Pair, def, record, req};
use crate::world::level::block;
use crate::world::level::light_layer::LightLayer;
use crate::world::level::LevelReader;
use crate::world::level::chunk::Biome;
use crate::world::level::levelgen::density_function::float;
use crate::world::level::levelgen::synth::{SimplexNoise, SimplexStack};
use climate::{ParameterList, ParameterPoint, TargetPoint};

#[derive(Clone, Copy)]
struct ClimateSettings {
    has_precipitation: bool,
    temperature: f32,
    frozen: bool,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Precipitation {
    None,
    Rain,
    Snow,
}

pub fn validate(tag: &Nbt) -> Result<(), String> {
    use crate::util::codec::{self, DecodeResult, Record};
    codec::map(tag)?;
    let mut record = Record::default();
    record.required(tag, "has_precipitation", codec::bool_value);
    record.required(tag, "temperature", codec::float);
    record.optional(tag, "temperature_modifier", |modifier| codec::string_enum(modifier, &[("none", ()), ("frozen", ())]));
    record.required(tag, "downfall", codec::float);
    record.optional(tag, "attributes", crate::world::attribute::environment_attribute_map::decode_only_positional);
    record.required(tag, "effects", |effects| biome_special_effects::codec().0.decode_partial(effects));
    record.required(tag, "carvers", |carvers| {
        let result = crate::world::level::levelgen::carver::list_codec().0.decode_partial(carvers);
        if let Some(error) = &result.error {
            crate::error!("Carver: {error}");
        }
        match result.value {
            Some(value) => DecodeResult::success(value),
            None => result,
        }
    });
    record.required(tag, "features", |features| {
        let result = crate::world::level::levelgen::placement::placed_feature::list_of_lists_codec().0.decode_partial(features);
        if let Some(error) = &result.error {
            crate::error!("Features: {error}");
        }
        match result.value {
            Some(value) => DecodeResult::success(value),
            None => result,
        }
    });
    record.build(|| Some(())).into_result()
}

fn network_codec() -> Pair {
    record(vec![
        req("has_precipitation", types::bool()),
        req("temperature", types::float(Codec::float())),
        def("temperature_modifier", types::enumeration(&["none", "frozen"]), types::s("none")),
        req("downfall", types::float(Codec::float())),
        req("effects", biome_special_effects::codec()),
    ])
}

pub fn encode_network(json: &Value) -> Result<Nbt, String> {
    let Nbt::Compound(mut fields) = network_codec().0.decode(&json_to_nbt(json))? else { return Err("A biome is not a compound".into()) };
    if let Some(attributes) = json.get("attributes") {
        let map = EnvironmentAttributeMap::parse(attributes);
        let attributes = map.encode_network()?;
        if !map.entries().is_empty() {
            fields.push(("attributes".into(), attributes));
        }
    }
    Ok(Nbt::Compound(fields))
}

static CLIMATE: LazyLock<Vec<ClimateSettings>> = LazyLock::new(|| {
    let registry = wafer_data::SyncedRegistry::get("minecraft:worldgen/biome");
    registry
        .entries
        .iter()
        .map(|name| {
            let source = crate::core::data_registries::json_entries("minecraft:worldgen/biome").iter().find(|(n, _)| n == name).map(|(_, s)| *s).expect("biome data");
            let json: Value = serde_json::from_str(source).expect("biome json");
            ClimateSettings {
                has_precipitation: json["has_precipitation"].as_bool().expect("biome precipitation"),
                temperature: float(&json["temperature"]).expect("biome temperature"),
                frozen: json.get("temperature_modifier").and_then(Value::as_str) == Some("frozen"),
            }
        })
        .collect()
});

pub static BIOME_ATTRIBUTES: LazyLock<Vec<EnvironmentAttributeMap>> = LazyLock::new(|| {
    let registry = wafer_data::SyncedRegistry::get("minecraft:worldgen/biome");
    registry
        .entries
        .iter()
        .map(|name| {
            let source = crate::core::data_registries::json_entries("minecraft:worldgen/biome").iter().find(|(n, _)| n == name).map(|(_, s)| *s).expect("biome data");
            let json: Value = serde_json::from_str(source).expect("biome json");
            match json.get("attributes") {
                Some(attributes) => EnvironmentAttributeMap::parse(attributes),
                None => EnvironmentAttributeMap::default(),
            }
        })
        .collect()
});

static TEMPERATURE_NOISE: LazyLock<SimplexNoise> = LazyLock::new(|| SimplexNoise::new(&mut LegacyRandomSource::new(1234), true));

static FROZEN_TEMPERATURE_NOISE: LazyLock<SimplexStack> = LazyLock::new(|| {
    let mut random = LegacyRandomSource::new(3456);
    let a = SimplexNoise::new(&mut random, true);
    let b = SimplexNoise::new(&mut random, true);
    let c = SimplexNoise::new(&mut random, true);
    SimplexStack::new(vec![(a, 1.0, 0.14285715), (b, 0.5, 0.2857143), (c, 0.25, 0.5714286)])
});

pub static BIOME_INFO_NOISE: LazyLock<SimplexNoise> = LazyLock::new(|| SimplexNoise::new(&mut LegacyRandomSource::new(2345), true));

impl Biome {
    fn climate(self) -> ClimateSettings {
        CLIMATE[self.0 as usize]
    }

    pub fn temperature(self, pos: BlockPos, sea_level: i32) -> f32 {
        let climate = self.climate();
        let mut adjusted = climate.temperature;
        if climate.frozen {
            let large = (FROZEN_TEMPERATURE_NOISE.get_2d(pos.x as f64 * 0.05, pos.z as f64 * 0.05) * 7.0) as f64;
            let edge = BIOME_INFO_NOISE.get_2d(pos.x as f64 * 0.2, pos.z as f64 * 0.2) as f64;
            if large + edge < 0.3 && (BIOME_INFO_NOISE.get_2d(pos.x as f64 * 0.09, pos.z as f64 * 0.09) as f64) < 0.8 {
                adjusted = 0.2;
            }
        }
        let snow_level = sea_level + 17;
        if pos.y > snow_level {
            let v = TEMPERATURE_NOISE.get_2d((pos.x as f32 / 8.0) as f64, (pos.z as f32 / 8.0) as f64) * 8.0;
            adjusted - (v + pos.y as f32 - snow_level as f32) * 0.05 / 40.0
        } else {
            adjusted
        }
    }

    pub fn cold_enough_to_snow(self, pos: BlockPos, sea_level: i32) -> bool {
        !self.warm_enough_to_rain(pos, sea_level)
    }

    pub fn warm_enough_to_rain(self, pos: BlockPos, sea_level: i32) -> bool {
        self.temperature(pos, sea_level) >= 0.15
    }

    pub fn precipitation_at(self, pos: BlockPos, sea_level: i32) -> Precipitation {
        if !self.climate().has_precipitation {
            Precipitation::None
        } else if self.cold_enough_to_snow(pos, sea_level) {
            Precipitation::Snow
        } else {
            Precipitation::Rain
        }
    }

    pub fn should_melt_frozen_ocean_iceberg_slightly(self, pos: BlockPos, sea_level: i32) -> bool {
        self.temperature(pos, sea_level) > 0.1
    }

    pub fn should_freeze(self, level: &mut dyn LevelReader, pos: BlockPos, check_neighbors: bool) -> bool {
        if self.warm_enough_to_rain(pos, level.get_sea_level()) {
            return false;
        }
        if level.is_inside_build_height(pos.y) && level.get_brightness(LightLayer::Block, pos) < 10 {
            let block_state = level.block_state(pos);
            let fluid_state = level.fluid_state(pos);
            if fluid_state.fluid == Fluid::Water && block_state.block().is_instance_of("LiquidBlock") {
                if !check_neighbors {
                    return true;
                }
                let surrounded_by_water = level.is_water_at(pos.west()) && level.is_water_at(pos.east()) && level.is_water_at(pos.north()) && level.is_water_at(pos.south());
                if !surrounded_by_water {
                    return true;
                }
            }
        }
        false
    }

    pub fn should_snow(self, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        if self.precipitation_at(pos, level.get_sea_level()) != Precipitation::Snow {
            return false;
        }
        if level.is_inside_build_height(pos.y) && level.get_brightness(LightLayer::Block, pos) < 10 {
            let state = level.block_state(pos);
            let snow = blocks::SNOW.default_state();
            if (state.is_air() || state.is_block(blocks::SNOW)) && block::of(snow).can_survive(snow, level, pos) {
                return true;
            }
        }
        false
    }
}

pub struct MultiNoiseBiomeSource {
    pub preset: String,
    pub parameters: ParameterList<Biome>,
}

pub fn decode_parameter_list(tag: &Nbt) -> Result<(), String> {
    use crate::util::codec::{self, Record};
    codec::map(tag)?;
    let mut record = Record::default();
    record.required(tag, "preset", |preset| {
        let name = codec::identifier(preset)?;
        if ["minecraft:nether", "minecraft:overworld"].contains(&name.as_str()) { Ok(()) } else { Err(format!("Unknown preset: {name}")) }
    });
    record.build(|| Some(())).into_result()
}

impl MultiNoiseBiomeSource {
    pub fn from_preset(name: &str) -> Result<MultiNoiseBiomeSource, String> {
        let source = crate::core::data_registries::json_entries("minecraft:worldgen/multi_noise_biome_source_parameter_list").iter().find(|(n, _)| *n == name).ok_or_else(|| format!("unknown preset {name}"))?.1;
        let json: Value = serde_json::from_str(source).map_err(|e| e.to_string())?;
        let preset = json.get("preset").and_then(Value::as_str).ok_or("preset")?;
        let report = wafer_data::BIOME_PARAMETERS.iter().find(|(n, _)| *n == preset).ok_or_else(|| format!("no parameters for {preset}"))?.1;
        let report: Value = serde_json::from_str(report).map_err(|e| e.to_string())?;
        let mut values = Vec::new();
        for entry in report["biomes"].as_array().ok_or("biomes")? {
            let biome_name = entry["biome"].as_str().ok_or("biome")?;
            let biome = Biome::by_name(biome_name).ok_or_else(|| format!("unknown biome {biome_name}"))?;
            values.push((ParameterPoint::parse_report(&entry["parameters"])?, biome));
        }
        Ok(MultiNoiseBiomeSource { preset: name.to_owned(), parameters: ParameterList::new(values) })
    }

    pub fn from_parameters(biomes: &Value) -> Result<MultiNoiseBiomeSource, String> {
        let mut values = Vec::new();
        for entry in biomes.as_array().ok_or("Not a list")? {
            let biome_name = entry["biome"].as_str().ok_or("No key biome")?;
            let biome = Biome::by_name(biome_name).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/biome]: {biome_name}"))?;
            values.push((ParameterPoint::parse_report(&entry["parameters"])?, biome));
        }
        Ok(MultiNoiseBiomeSource { preset: String::new(), parameters: ParameterList::new(values) })
    }

    pub fn get_noise_biome(&self, target: &TargetPoint) -> Biome {
        *self.parameters.find_value(target)
    }

    pub fn possible_biomes(&self) -> Vec<Biome> {
        let mut biomes: Vec<Biome> = Vec::new();
        for &(_, biome) in self.parameters.values() {
            if !biomes.contains(&biome) {
                biomes.push(biome);
            }
        }
        biomes
    }
}

fn lcg_next(rval: i64, c: i64) -> i64 {
    rval.wrapping_mul(rval.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407)).wrapping_add(c)
}

fn fiddle(rval: i64) -> f64 {
    let uniform = (rval >> 24).rem_euclid(1024) as f64 / 1024.0;
    (uniform - 0.5) * 0.9
}

fn fiddled_distance(seed: i64, x: i32, y: i32, z: i32, dx: f64, dy: f64, dz: f64) -> f64 {
    let mut rval = seed;
    for c in [x, y, z, x, y, z] {
        rval = lcg_next(rval, c as i64);
    }
    let fx = fiddle(rval);
    rval = lcg_next(rval, seed);
    let fy = fiddle(rval);
    rval = lcg_next(rval, seed);
    let fz = fiddle(rval);
    (dz + fz) * (dz + fz) + (dy + fy) * (dy + fy) + (dx + fx) * (dx + fx)
}

pub fn zoomed_quart(zoom_seed: i64, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    let (ax, ay, az) = (x - 2, y - 2, z - 2);
    let (px, py, pz) = (ax >> 2, ay >> 2, az >> 2);
    let (fx, fy, fz) = ((ax & 3) as f64 / 4.0, (ay & 3) as f64 / 4.0, (az & 3) as f64 / 4.0);
    let mut min_i = 0;
    let mut min_distance = f64::INFINITY;
    for i in 0..8 {
        let (xe, ye, ze) = (i & 4 == 0, i & 2 == 0, i & 1 == 0);
        let distance = fiddled_distance(
            zoom_seed,
            if xe { px } else { px + 1 },
            if ye { py } else { py + 1 },
            if ze { pz } else { pz + 1 },
            if xe { fx } else { fx - 1.0 },
            if ye { fy } else { fy - 1.0 },
            if ze { fz } else { fz - 1.0 },
        );
        if min_distance > distance {
            min_i = i;
            min_distance = distance;
        }
    }
    (if min_i & 4 == 0 { px } else { px + 1 }, if min_i & 2 == 0 { py } else { py + 1 }, if min_i & 1 == 0 { pz } else { pz + 1 })
}

pub fn obfuscate_seed(seed: i64) -> i64 {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(seed.to_le_bytes());
    i64::from_le_bytes(hash[..8].try_into().unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn validation_error(json: &str) -> String {
        let value = crate::util::gson::parse_lenient(json).unwrap();
        crate::nbt::with_json_source(|| validate(&crate::world::item::component::element_to_nbt(&value)).err().expect("a validation error"))
    }

    fn attribute_error(attributes: &str) -> String {
        let json = format!(r#"{{"has_precipitation":true,"temperature":0.5,"downfall":0.5,"effects":{{"water_color":4159204}},"carvers":[],"features":[],"attributes":{attributes}}}"#);
        validation_error(&json)
    }

    #[test]
    fn biome_attributes_report_the_vanilla_messages() {
        let unknown = "Unknown registry key in ResourceKey[minecraft:root / minecraft:environment_attribute]: minecraft:x";
        assert_eq!(attribute_error(r#"{"x":1}"#), format!(r#"{unknown}; {unknown} missed input: {{"x":1}}"#));
        assert_eq!(attribute_error("[]"), "Not a JSON object: []");
        assert_eq!(
            attribute_error(r##"{"minecraft:visual/fog_color":"#ff0000","visual/fog_color":"#00ff00"}"##),
            r##"Duplicate entry for key: 'minecraft:visual/fog_color' missed input: {"visual/fog_color":"#00ff00"}"##
        );
        assert_eq!(
            attribute_error(r#"{"minecraft:visual/fog_color":[0.5,0.5,0.5,0.5]}"#),
            r#"Input is not a list of 3 elements missed input: {"minecraft:visual/fog_color":[0.5,0.5,0.5,0.5]}"#
        );
        assert_eq!(
            attribute_error(r#"{"minecraft:gameplay/snow_golem_melts":1}"#),
            r#"Failed to parse either. First: Not a boolean: 1; Second: Not a JSON object: 1 missed input: {"minecraft:gameplay/snow_golem_melts":1}"#
        );
    }

    #[test]
    fn a_biome_reports_every_field_error_in_vanilla_order() {
        assert_eq!(
            validation_error("{}"),
            "No key features in MapLike[{}]; No key carvers in MapLike[{}]; No key effects in MapLike[{}]; No key downfall in MapLike[{}]; No key temperature in MapLike[{}]; No key has_precipitation in MapLike[{}]"
        );
        let json = r##"{"temperature":"x","downfall":0.5,"has_precipitation":"maybe","effects":{"water_color":"x"},"carvers":[],"features":"y"}"##;
        assert_eq!(
            validation_error(json),
            "Not a json array: \"y\"; Failed to parse either. First: Hex color must begin with #; Second: Failed to parse either. First: Not a number: \"x\"; Second: Not a json array: \"x\"; Not a number: \"x\"; Not a boolean: \"maybe\""
        );
    }
}
