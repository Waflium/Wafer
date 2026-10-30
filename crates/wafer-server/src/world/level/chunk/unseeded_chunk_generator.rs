use std::sync::Arc;

use wafer_data::BlockState;
use wafer_protocol::nbt::Nbt;

use super::{Biome, ChunkGenerator, FlatLevelSource};
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::item::component::codec::{Codec, RangeError};
use crate::world::level::biome::MultiNoiseBiomeSource;
use crate::world::level::biome::biome_source::{BiomeSource, CheckerboardColumnBiomeSource};
use crate::world::level::biome::the_end_biome_source::TheEndBiomeSource;
use crate::world::level::levelgen::debug_level_source::DebugLevelSource;
use crate::world::level::levelgen::{NoiseBasedChunkGenerator, NoiseGeneratorSettings, WORLDGEN, flat};

const CHUNK_GENERATOR_TYPES: [&str; 3] = ["minecraft:noise", "minecraft:flat", "minecraft:debug"];
const BIOME_SOURCE_TYPES: [&str; 4] = ["minecraft:fixed", "minecraft:multi_noise", "minecraft:checkerboard", "minecraft:the_end"];

#[derive(Clone)]
pub enum NoiseSettingsHolder {
    Reference(String),
    Direct(Arc<NoiseGeneratorSettings>, Nbt),
}

#[derive(Clone)]
pub enum BiomeSourceDefinition {
    MultiNoisePreset(String),
    Built(Arc<BiomeSource>, Nbt),
}

impl BiomeSourceDefinition {
    pub fn build(&self) -> Arc<BiomeSource> {
        match self {
            BiomeSourceDefinition::MultiNoisePreset(preset) => Arc::new(BiomeSource::MultiNoise(MultiNoiseBiomeSource::from_preset(preset).expect("a multi noise biome source preset"))),
            BiomeSourceDefinition::Built(source, _) => source.clone(),
        }
    }

    fn encode(&self) -> Nbt {
        match self {
            BiomeSourceDefinition::MultiNoisePreset(preset) => Nbt::Compound(vec![("preset".into(), Nbt::String(preset.clone())), ("type".into(), Nbt::String("minecraft:multi_noise".into()))].into()),
            BiomeSourceDefinition::Built(_, tag) => tag.clone(),
        }
    }
}

#[derive(Clone)]
pub struct NoiseGeneratorDefinition {
    pub settings: NoiseSettingsHolder,
    pub biome_source: BiomeSourceDefinition,
}

impl NoiseGeneratorDefinition {
    pub fn stable(&self, expected: &str) -> bool {
        matches!(&self.settings, NoiseSettingsHolder::Reference(key) if key == expected)
    }

    pub fn is_the_end_biome_source(&self) -> bool {
        matches!(&self.biome_source, BiomeSourceDefinition::Built(source, _) if matches!(**source, BiomeSource::TheEnd(_)))
    }

    pub fn multi_noise_biome_source_stable(&self, expected: &str) -> Option<bool> {
        match &self.biome_source {
            BiomeSourceDefinition::MultiNoisePreset(preset) => Some(preset == expected),
            BiomeSourceDefinition::Built(source, _) => matches!(**source, BiomeSource::MultiNoise(_)).then_some(false),
        }
    }
}

#[derive(Clone)]
pub enum UnseededChunkGenerator {
    Flat(FlatLevelSource),
    Noise(NoiseGeneratorDefinition),
    Debug,
}

impl UnseededChunkGenerator {
    pub fn is_debug(&self) -> bool {
        matches!(self, UnseededChunkGenerator::Debug)
    }

    pub fn is_flat(&self) -> bool {
        matches!(self, UnseededChunkGenerator::Flat(_))
    }

    pub fn get_type_name_for_data_fixer(&self) -> &'static str {
        match self {
            UnseededChunkGenerator::Flat(_) => "minecraft:flat",
            UnseededChunkGenerator::Debug => "minecraft:debug",
            UnseededChunkGenerator::Noise(_) => "minecraft:noise",
        }
    }

    pub fn create(&self, seed: i64) -> ChunkGenerator {
        match self {
            UnseededChunkGenerator::Flat(flat) => ChunkGenerator::Flat(flat.clone()),
            UnseededChunkGenerator::Debug => ChunkGenerator::Debug(DebugLevelSource),
            UnseededChunkGenerator::Noise(noise) => ChunkGenerator::Noise(Arc::new(match &noise.settings {
                NoiseSettingsHolder::Reference(key) => NoiseBasedChunkGenerator::with_settings(key, noise.biome_source.build(), seed),
                NoiseSettingsHolder::Direct(settings, _) => NoiseBasedChunkGenerator::new(settings.clone(), noise.biome_source.build(), seed),
            })),
        }
    }

    pub fn decode(tag: &Nbt) -> DecodeResult<UnseededChunkGenerator> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let ty = match decode_dispatch_type(tag, "minecraft:worldgen/chunk_generator", &CHUNK_GENERATOR_TYPES) {
            Ok(ty) => ty,
            Err(error) => return DecodeResult::error(error),
        };
        let mut record = Record::default();
        match ty.as_str() {
            "minecraft:noise" => {
                let biome_source = record.required(tag, "biome_source", decode_biome_source);
                let settings = record.required(tag, "settings", decode_noise_settings);
                record.build(|| Some(UnseededChunkGenerator::Noise(NoiseGeneratorDefinition { settings: settings?, biome_source: biome_source? })))
            }
            "minecraft:flat" => {
                let settings = record.required(tag, "settings", flat::decode_settings);
                record.build(|| settings.map(UnseededChunkGenerator::Flat))
            }
            _ => record.build(|| Some(UnseededChunkGenerator::Debug)),
        }
    }

    pub fn encode(&self) -> Nbt {
        let string = |value: &str| Nbt::String(value.to_string());
        match self {
            UnseededChunkGenerator::Flat(flat) => {
                let layers = flat
                    .layers
                    .iter()
                    .map(|&(state, height): &(BlockState, u32)| Nbt::Compound(vec![("block".into(), string(state.block().name())), ("height".into(), Nbt::Int(height as i32))].into()))
                    .collect();
                let biome = if flat.biome.is_inline() { crate::world::level::biome::inline_biome::get(flat.biome).tag.clone() } else { string(flat.biome.name()) };
                let mut settings = vec![("features".to_string(), Nbt::Byte(flat.features as i8)), ("biome".into(), biome), ("layers".into(), Nbt::List(layers))];
                if let Some(overrides) = &flat.structure_overrides {
                    settings.push(("structure_overrides".into(), overrides.to_nbt()));
                }
                settings.push(("lakes".into(), Nbt::Byte(flat.lakes as i8)));
                Nbt::Compound(vec![("settings".into(), Nbt::Compound(settings.into())), ("type".into(), string("minecraft:flat"))].into())
            }
            UnseededChunkGenerator::Noise(noise) => Nbt::Compound(vec![
                (
                    "settings".into(),
                    match &noise.settings {
                        NoiseSettingsHolder::Reference(key) => string(key),
                        NoiseSettingsHolder::Direct(_, tag) => tag.clone(),
                    },
                ),
                ("biome_source".into(), noise.biome_source.encode()),
                ("type".into(), string("minecraft:noise")),
            ].into()),
            UnseededChunkGenerator::Debug => Nbt::Compound(vec![("type".into(), string("minecraft:debug"))].into()),
        }
    }
}

fn holder(registry: &'static str) -> Codec {
    Codec::Holder(registry, None)
}

fn decode_dispatch_type(tag: &Nbt, registry: &str, types: &[&str]) -> Result<String, String> {
    let ty = codec::identifier(codec::field(tag, "type")?)?;
    if types.contains(&ty.as_str()) { Ok(ty) } else { Err(format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {ty}")) }
}

fn decode_noise_settings(tag: &Nbt) -> DecodeResult<NoiseSettingsHolder> {
    let DecodeResult { value, error } = crate::world::level::levelgen::noise_settings_holder_codec().0.decode_partial(tag);
    let value = value.and_then(|value| match value {
        Nbt::String(key) => Some(NoiseSettingsHolder::Reference(key)),
        direct => crate::nbt::with_json_source(|| WORLDGEN.parse_inline_settings(&codec::to_json(&direct))).ok().map(|settings| NoiseSettingsHolder::Direct(settings, direct)),
    });
    DecodeResult { value, error }
}

fn decode_biome_source(tag: &Nbt) -> DecodeResult<BiomeSourceDefinition> {
    if let Err(error) = codec::map(tag) {
        return DecodeResult::error(error);
    }
    let ty = match decode_dispatch_type(tag, "minecraft:worldgen/biome_source", &BIOME_SOURCE_TYPES) {
        Ok(ty) => ty,
        Err(error) => return DecodeResult::error(error),
    };
    let ty_tag = Nbt::String(ty.clone());
    let mut record = Record::default();
    match ty.as_str() {
        "minecraft:fixed" => {
            let biome = record.required(tag, "biome", |biome| DecodeResult::from(holder("minecraft:worldgen/biome").decode(biome)));
            record.build(|| {
                let biome = Biome::by_name(biome?.as_str()?)?;
                let encoded = Nbt::Compound(vec![("biome".into(), Nbt::String(biome.name().to_string())), ("type".into(), ty_tag)].into());
                Some(BiomeSourceDefinition::Built(Arc::new(BiomeSource::Fixed(biome)), encoded))
            })
        }
        "minecraft:checkerboard" => {
            let biomes = record.required(tag, "biomes", |biomes| DecodeResult::from(Codec::HolderSet("minecraft:worldgen/biome").decode(biomes)));
            let scale = record.with_default(tag, "scale", 2, |scale| Codec::Int(0, 62, RangeError::Outside).decode(scale).map(|scale| scale.as_i64().unwrap_or(2) as i32));
            record.build(|| {
                let biomes = biomes?;
                let allowed = crate::core::holder_set::HolderSet::decode("minecraft:worldgen/biome", &biomes).ok()?.contents().iter().map(|&id| Biome(id as u16)).collect();
                let mut fields = vec![("biomes".to_string(), biomes)];
                if scale != 2 {
                    fields.push(("scale".into(), Nbt::Int(scale)));
                }
                fields.push(("type".into(), ty_tag));
                Some(BiomeSourceDefinition::Built(Arc::new(BiomeSource::Checkerboard(CheckerboardColumnBiomeSource::new(allowed, scale))), Nbt::Compound(fields.into())))
            })
        }
        "minecraft:multi_noise" => decode_multi_noise(tag, ty_tag),
        _ => record.build(|| Some(BiomeSourceDefinition::Built(Arc::new(BiomeSource::TheEnd(TheEndBiomeSource::create())), Nbt::Compound(vec![("type".into(), ty_tag)].into())))),
    }
}

fn decode_multi_noise(tag: &Nbt, ty_tag: Nbt) -> DecodeResult<BiomeSourceDefinition> {
    let parameters = match tag.get("biomes") {
        Some(biomes) => match MultiNoiseBiomeSource::from_parameters(&codec::to_json(biomes)) {
            Ok(source) => DecodeResult::success(source),
            Err(error) => DecodeResult::error(error),
        },
        None => DecodeResult::error(codec::no_key("biomes", &codec::to_json(tag))),
    };
    let DecodeResult { value: parameters_value, error: parameters_error } = parameters;
    if let (Some(source), None) = (parameters_value, &parameters_error) {
        let biomes = tag.get("biomes").cloned().unwrap_or(Nbt::End);
        return DecodeResult::success(BiomeSourceDefinition::Built(Arc::new(BiomeSource::MultiNoise(source)), Nbt::Compound(vec![("biomes".into(), biomes), ("type".into(), ty_tag)].into())));
    }
    let preset = match tag.get("preset") {
        Some(preset) => DecodeResult::from(holder("minecraft:worldgen/multi_noise_biome_source_parameter_list").decode(preset)),
        None => DecodeResult::error(codec::no_key("preset", &codec::to_json(tag))),
    };
    match preset {
        DecodeResult { value: Some(Nbt::String(name)), error: None } => DecodeResult::success(BiomeSourceDefinition::MultiNoisePreset(name)),
        failed => DecodeResult::error(format!("{}; {}", failed.error.unwrap_or_default(), parameters_error.unwrap_or_default())),
    }
}
