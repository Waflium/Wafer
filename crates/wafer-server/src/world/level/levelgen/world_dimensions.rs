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

fn biome_source_from_json(json: &Value) -> Result<BiomeSource, String> {
    match json["type"].as_str() {
        Some("minecraft:multi_noise") => {
            let preset = json["preset"].as_str().ok_or("Only multi noise biome source presets are supported")?;
            Ok(BiomeSource::MultiNoise(MultiNoiseBiomeSource::from_preset(preset)?))
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
            let settings = json["settings"].as_str().ok_or("Only named noise settings are supported")?;
            if !WORLDGEN.noise_settings.contains_key(settings) {
                return Err(format!("Unknown noise settings {settings}"));
            }
            let biome_source = biome_source_from_json(&json["biome_source"])?;
            Ok(ChunkGenerator::Noise(Arc::new(NoiseBasedChunkGenerator::with_settings(settings, biome_source, seed))))
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
