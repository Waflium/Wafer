use std::sync::Arc;

use serde_json::Value;
use wafer_data::{Block, BlockState};
use wafer_protocol::nbt::Nbt;

use super::debug_level_source::DebugLevelSource;
use super::{NoiseBasedChunkGenerator, WORLDGEN, flat};
use crate::world::level::biome::MultiNoiseBiomeSource;
use crate::world::level::biome::biome_source::BiomeSource;
use crate::world::level::biome::the_end_biome_source::TheEndBiomeSource;
use crate::world::level::chunk::{Biome, ChunkGenerator, FlatLevelSource};
use super::flat::StructureOverrides;
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::item::component::codec::{Codec, RangeError};
use crate::world::item::component::types;
use crate::world::level::dimension::dimension_type;

#[derive(Clone)]
pub struct WorldDimensions {
    pub overworld: ChunkGenerator,
    pub nether: ChunkGenerator,
    pub end: ChunkGenerator,
}

impl WorldDimensions {
    pub fn normal(seed: i64) -> WorldDimensions {
        WorldDimensions {
            overworld: ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::overworld(seed))),
            nether: ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::nether(seed))),
            end: ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::end(seed))),
        }
    }

    pub fn from_preset(preset: &str, seed: i64) -> Result<WorldDimensions, String> {
        let json = crate::core::data_registries::json_entries("minecraft:worldgen/world_preset").iter().find(|(name, _)| *name == preset).ok_or_else(|| format!("Unknown world preset {preset}"))?.1;
        let json: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
        WorldDimensions::from_json(&json["dimensions"], seed)
    }

    pub fn stem_from_json(stem: &Value, seed: i64) -> Result<(String, ChunkGenerator), String> {
        let dimension_type = stem["type"].as_str().ok_or("No key type")?;
        if wafer_data::SyncedRegistry::get("minecraft:dimension_type").id_of(dimension_type).is_none() {
            return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:dimension_type]: {dimension_type}"));
        }
        Ok((dimension_type.to_owned(), generator_from_json(&stem["generator"], seed)?))
    }

    pub fn from_json(dimensions: &Value, seed: i64) -> Result<WorldDimensions, String> {
        let generator = |name: &str| generator_from_json(&dimensions[name]["generator"], seed);
        Ok(WorldDimensions { overworld: generator("minecraft:overworld")?, nether: generator("minecraft:the_nether")?, end: generator("minecraft:the_end")? })
    }

    pub fn from_nbt(dimensions: &Nbt, seed: i64) -> Result<WorldDimensions, String> {
        let generator = |name: &str| {
            let generator = dimensions.get(name).and_then(|d| d.get("generator")).ok_or_else(|| format!("No key {name}"))?;
            generator_from_nbt(generator, seed)
        };
        Ok(WorldDimensions { overworld: generator("minecraft:overworld")?, nether: generator("minecraft:the_nether")?, end: generator("minecraft:the_end")? })
    }

    pub fn is_debug(&self) -> bool {
        matches!(self.overworld, ChunkGenerator::Debug(_))
    }

    pub fn to_nbt(&self) -> Nbt {
        let stem = |generator: &ChunkGenerator, ty: &str| Nbt::Compound(vec![("generator".into(), generator_to_nbt(generator)), ("type".into(), Nbt::String(ty.into()))]);
        Nbt::Compound(vec![
            ("minecraft:overworld".into(), stem(&self.overworld, "minecraft:overworld")),
            ("minecraft:the_nether".into(), stem(&self.nether, "minecraft:the_nether")),
            ("minecraft:the_end".into(), stem(&self.end, "minecraft:the_end")),
        ])
    }
}

const CHUNK_GENERATOR_TYPES: [&str; 3] = ["minecraft:noise", "minecraft:flat", "minecraft:debug"];
const BIOME_SOURCE_TYPES: [&str; 4] = ["minecraft:fixed", "minecraft:multi_noise", "minecraft:checkerboard", "minecraft:the_end"];

fn holder(registry: &'static str) -> Codec {
    Codec::Holder(registry, None)
}

fn decode_dispatch_type(tag: &Nbt, registry: &str, types: &[&str]) -> Result<String, String> {
    let ty = codec::identifier(codec::field(tag, "type")?)?;
    if types.contains(&ty.as_str()) { Ok(ty) } else { Err(format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {ty}")) }
}

pub fn decode_level_stem(tag: &Nbt) -> DecodeResult<()> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let ty = record.required(tag, "type", |ty| DecodeResult::from(types::holder("minecraft:dimension_type", dimension_type::direct_codec()).0.decode(ty)));
    let generator = record.required(tag, "generator", decode_chunk_generator);
    record.build(|| ty.and(generator))
}

pub fn decode_world_preset(tag: &Nbt) -> DecodeResult<()> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let mut record = Record::default();
    let dimensions = record.required(tag, "dimensions", |dimensions| {
        codec::base_map_partial(dimensions, |key| codec::identifier(&Nbt::String(key.to_string())), decode_level_stem)
    });
    record.build(|| dimensions).validate(|dimensions| {
        if dimensions.iter().any(|(key, _)| key == "minecraft:overworld") { Ok(()) } else { Err("Missing overworld dimension".to_string()) }
    })
    .map(drop)
}

fn decode_chunk_generator(tag: &Nbt) -> DecodeResult<()> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let ty = match decode_dispatch_type(tag, "minecraft:worldgen/chunk_generator", &CHUNK_GENERATOR_TYPES) {
        Ok(ty) => ty,
        Err(error) => return DecodeResult::error(error),
    };
    let mut record = Record::default();
    let decoded = match ty.as_str() {
        "minecraft:noise" => {
            let biome_source = record.required(tag, "biome_source", decode_biome_source);
            let settings = record.required(tag, "settings", decode_noise_settings);
            biome_source.and(settings)
        }
        "minecraft:flat" => record.required(tag, "settings", |settings| {
            let parsed = flat::parse_settings(&codec::to_json(settings));
            DecodeResult { value: parsed.value.map(drop), error: parsed.error }
        }),
        _ => Some(()),
    };
    record.build(|| decoded)
}

fn decode_noise_settings(tag: &Nbt) -> DecodeResult<()> {
    match tag {
        Nbt::String(_) => DecodeResult::from(holder("minecraft:worldgen/noise_settings").decode(tag).map(drop)),
        inline => DecodeResult::from(super::validate("minecraft:worldgen/noise_settings", &codec::to_json(inline))),
    }
}

fn decode_biome_source(tag: &Nbt) -> DecodeResult<()> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let ty = match decode_dispatch_type(tag, "minecraft:worldgen/biome_source", &BIOME_SOURCE_TYPES) {
        Ok(ty) => ty,
        Err(error) => return DecodeResult::error(error),
    };
    let mut record = Record::default();
    let decoded = match ty.as_str() {
        "minecraft:fixed" => record.required(tag, "biome", |biome| DecodeResult::from(holder("minecraft:worldgen/biome").decode(biome).map(drop))),
        "minecraft:checkerboard" => {
            let biomes = record.required(tag, "biomes", |biomes| DecodeResult::from(Codec::HolderSet("minecraft:worldgen/biome").decode(biomes).map(drop)));
            record.with_default(tag, "scale", 2, |scale| Codec::Int(0, 62, RangeError::Outside).decode(scale).map(|scale| scale.as_i64().unwrap_or(2) as i32));
            biomes
        }
        "minecraft:multi_noise" => return decode_multi_noise(tag),
        _ => Some(()),
    };
    record.build(|| decoded)
}

fn decode_multi_noise(tag: &Nbt) -> DecodeResult<()> {
    let biomes = match tag.get("biomes") {
        Some(biomes) => DecodeResult::from(MultiNoiseBiomeSource::from_parameters(&codec::to_json(biomes)).map(drop)),
        None => DecodeResult::error(codec::no_key("biomes", &codec::to_json(tag))),
    };
    if biomes.is_success() {
        return biomes;
    }
    let preset = match tag.get("preset") {
        Some(preset) => DecodeResult::from(holder("minecraft:worldgen/multi_noise_biome_source_parameter_list").decode(preset).map(drop)),
        None => DecodeResult::error(codec::no_key("preset", &codec::to_json(tag))),
    };
    if preset.is_success() {
        return preset;
    }
    DecodeResult::error(format!("{}; {}", preset.error.unwrap_or_default(), biomes.error.unwrap_or_default()))
}

fn biome_source_from_json(json: &Value) -> Result<BiomeSource, String> {
    match json["type"].as_str() {
        Some("minecraft:multi_noise") => match json["preset"].as_str() {
            Some(preset) => Ok(BiomeSource::MultiNoise(MultiNoiseBiomeSource::from_preset(preset)?)),
            None => Ok(BiomeSource::MultiNoise(MultiNoiseBiomeSource::from_parameters(&json["biomes"])?)),
        },
        Some("minecraft:checkerboard") => {
            let biomes = crate::core::holder_set::HolderSet::decode("minecraft:worldgen/biome", &crate::world::item::component::json_to_nbt(&json["biomes"]))?;
            let allowed: Vec<Biome> = biomes.contents().iter().map(|&id| Biome(id as u16)).collect();
            if allowed.is_empty() {
                return Err("Empty biome list".into());
            }
            Ok(BiomeSource::Checkerboard(crate::world::level::biome::biome_source::CheckerboardColumnBiomeSource::new(allowed, json["scale"].as_i64().unwrap_or(2) as i32)))
        }
        Some("minecraft:the_end") => Ok(BiomeSource::TheEnd(TheEndBiomeSource::create())),
        Some("minecraft:fixed") => {
            let biome = json["biome"].as_str().ok_or("No key biome")?;
            Ok(BiomeSource::Fixed(Biome::by_name(biome).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/biome]: {biome}"))?))
        }
        other => Err(format!("Unknown biome source type {other:?}")),
    }
}

pub fn generator_from_json(json: &Value, seed: i64) -> Result<ChunkGenerator, String> {
    match json["type"].as_str() {
        Some("minecraft:flat") => {
            let parsed = flat::parse_settings(&json["settings"]);
            parsed.value.ok_or_else(|| parsed.error.unwrap_or_default()).map(ChunkGenerator::Flat)
        }
        Some("minecraft:debug") => Ok(ChunkGenerator::Debug(DebugLevelSource)),
        Some("minecraft:noise") => {
            let biome_source = biome_source_from_json(&json["biome_source"])?;
            match &json["settings"] {
                Value::String(settings) => {
                    if !WORLDGEN.noise_settings.contains_key(settings) {
                        return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/noise_settings]: {settings}"));
                    }
                    Ok(ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::with_settings(settings, biome_source, seed))))
                }
                inline => Ok(ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::new(WORLDGEN.parse_inline_settings(inline)?, biome_source, seed)))),
            }
        }
        other => Err(format!("Unknown chunk generator type {other:?}")),
    }
}

fn generator_from_nbt(generator: &Nbt, seed: i64) -> Result<ChunkGenerator, String> {
    if generator.get("type").and_then(Nbt::as_str) != Some("minecraft:flat") {
        return generator_from_json(&crate::util::codec::to_json(generator), seed);
    }
    let settings = generator.get("settings").ok_or("flat settings missing")?;
    let mut layers = Vec::new();
    for layer in settings.get("layers").map(Nbt::as_list).unwrap_or(&[]) {
        let name = layer.get("block").and_then(Nbt::as_str).unwrap_or("minecraft:air");
        let state = Block::by_name(name).map(Block::default_state).ok_or_else(|| format!("unknown block {name}"))?;
        layers.push((state, layer.get("height").and_then(Nbt::as_i64).unwrap_or(1) as u32));
    }
    Ok(ChunkGenerator::Flat(FlatLevelSource {
        layers,
        biome: settings.get("biome").and_then(Nbt::as_str).and_then(Biome::by_name).unwrap_or(FlatLevelSource::default().biome),
        structure_overrides: settings.get("structure_overrides").and_then(StructureOverrides::from_nbt),
        lakes: settings.get("lakes").and_then(Nbt::as_i64).is_some_and(|v| v != 0),
        features: settings.get("features").and_then(Nbt::as_i64).is_some_and(|v| v != 0),
        generation: Default::default(),
    }))
}

fn biome_source_to_nbt(source: &BiomeSource) -> Nbt {
    match source {
        BiomeSource::MultiNoise(source) => Nbt::Compound(vec![("preset".into(), Nbt::String(source.preset.clone())), ("type".into(), Nbt::String("minecraft:multi_noise".into()))]),
        BiomeSource::TheEnd(_) => Nbt::Compound(vec![("type".into(), Nbt::String("minecraft:the_end".into()))]),
        BiomeSource::Fixed(biome) => Nbt::Compound(vec![("biome".into(), Nbt::String(biome.name().into())), ("type".into(), Nbt::String("minecraft:fixed".into()))]),
        BiomeSource::Checkerboard(source) => Nbt::Compound(vec![
            ("biomes".into(), Nbt::List(source.allowed_biomes.iter().map(|biome| Nbt::String(biome.name().into())).collect())),
            ("scale".into(), Nbt::Int(source.size)),
            ("type".into(), Nbt::String("minecraft:checkerboard".into())),
        ]),
    }
}

pub fn generator_to_nbt(generator: &ChunkGenerator) -> Nbt {
    match generator {
        ChunkGenerator::Flat(flat) => {
            let layers = flat
                .layers
                .iter()
                .map(|&(state, height): &(BlockState, u32)| {
                    Nbt::Compound(vec![("block".into(), Nbt::String(state.block().name().into())), ("height".into(), Nbt::Int(height as i32))])
                })
                .collect();
            let mut settings = vec![("features".to_string(), Nbt::Byte(flat.features as i8)), ("biome".into(), Nbt::String(flat.biome.name().into())), ("layers".into(), Nbt::List(layers))];
            if let Some(overrides) = &flat.structure_overrides {
                settings.push(("structure_overrides".into(), overrides.to_nbt()));
            }
            settings.push(("lakes".into(), Nbt::Byte(flat.lakes as i8)));
            Nbt::Compound(vec![("settings".into(), Nbt::Compound(settings)), ("type".into(), Nbt::String("minecraft:flat".into()))])
        }
        ChunkGenerator::Noise(noise) => Nbt::Compound(vec![
            ("settings".into(), Nbt::String(noise.settings_key.clone())),
            ("biome_source".into(), biome_source_to_nbt(&noise.biome_source)),
            ("type".into(), Nbt::String("minecraft:noise".into())),
        ]),
        ChunkGenerator::Debug(_) => Nbt::Compound(vec![("type".into(), Nbt::String("minecraft:debug".into()))]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::item::component::json_to_nbt;

    fn stem_error(json: &str) -> String {
        let value: Value = serde_json::from_str(json).unwrap();
        crate::nbt::with_json_source(|| decode_level_stem(&json_to_nbt(&value)).into_result().err().expect("a decode error"))
    }

    fn preset_error(json: &str) -> String {
        let value: Value = serde_json::from_str(json).unwrap();
        crate::nbt::with_json_source(|| decode_world_preset(&json_to_nbt(&value)).into_result().err().expect("a decode error"))
    }

    #[test]
    fn a_stem_reports_its_fields_in_vanilla_order() {
        assert_eq!(stem_error("{}"), "No key generator in MapLike[{}]; No key type in MapLike[{}]");
        assert_eq!(stem_error(r#"{"type":"ta:nope","generator":{"type":"minecraft:nope"}}"#), "Unknown registry key in ResourceKey[minecraft:root / minecraft:worldgen/chunk_generator]: minecraft:nope; Failed to get element ta:nope from registry minecraft:dimension_type");
        assert_eq!(stem_error(r#"{"type":"minecraft:overworld","generator":{"type":"minecraft:noise"}}"#), r#"No key settings in MapLike[{"type":"minecraft:noise"}]; No key biome_source in MapLike[{"type":"minecraft:noise"}]"#);
    }

    #[test]
    fn biome_sources_report_dispatch_and_field_errors() {
        let noise = |source: &str| format!(r#"{{"type":"minecraft:overworld","generator":{{"type":"minecraft:noise","settings":"minecraft:overworld","biome_source":{source}}}}}"#);
        assert!(stem_error(&noise(r#"{}"#)).ends_with("No key type in MapLike[{}]"));
        assert!(stem_error(&noise(r#"{"type":"minecraft:fixed","biome":"minecraft:nope"}"#)).ends_with("Failed to get element minecraft:nope from registry minecraft:worldgen/biome"));
        assert!(stem_error(&noise(r#"{"type":"minecraft:checkerboard","biomes":"minecraft:plains","scale":99}"#)).ends_with("Value 99 outside of range [0:62]"));
        assert!(stem_error(&noise(r#"{"type":"minecraft:multi_noise"}"#))
            .ends_with(r#"No key preset in MapLike[{"type":"minecraft:multi_noise"}]; No key biomes in MapLike[{"type":"minecraft:multi_noise"}]"#));
    }

    #[test]
    fn a_world_preset_needs_an_overworld() {
        assert_eq!(preset_error(r#"{"dimensions":{}}"#), "Missing overworld dimension");
        assert_eq!(preset_error("{}"), "No key dimensions in MapLike[{}]");
        assert_eq!(
            preset_error(r#"{"dimensions":{"ta:x":{"type":"minecraft:overworld"}}}"#),
            r#"No key generator in MapLike[{"type":"minecraft:overworld"}] missed input: {"ta:x":{"type":"minecraft:overworld"}}; Missing overworld dimension"#
        );
    }
}
