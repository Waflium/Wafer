use std::sync::Arc;

use serde_json::Value as Json;
use wafer_protocol::nbt::Nbt;

use super::modifier::color_modifier::{parse_argb, parse_rgb};
use super::modifier::{AttributeModifier, BooleanModifier, ColorModifier, FloatModifier, IntegerModifier, OperationId};
use super::{AmbientParticle, BedRule, LerpFunction, Value, ambient_sounds, background_music};
use crate::core::particles::particle_types;
use crate::util::argb;
use crate::util::extra_codecs::hex_color;
use crate::util::tri_state::TriState;
use wafer_protocol::{Reader, Writer};

use crate::util::hash_ops::{self, HashCode};
use crate::util::valueproviders::int_provider_codec;
use crate::world::item::component::codec::{Codec, Custom, Stream};
use crate::world::item::component::json_to_nbt;
use crate::world::item::component::text;
use crate::world::item::component::types::{self, Pair};
use crate::world::entity::schedule::Activity;
use crate::world::level::biome::mob_spawn_settings::MobSpawnSettings;
use crate::world::level::levelgen::density_function::{float, resource_name};
use crate::world::level::moon_phase::MoonPhase;

const MOB_CATEGORIES: [&str; 8] = ["monster", "creature", "ambient", "axolotls", "underground_water_creature", "water_creature", "water_ambient", "misc"];

fn check_hex_color<const DIGITS: usize>(value: Nbt) -> Result<Nbt, String> {
    let string = value.as_str().unwrap_or_default();
    let Some(hex) = string.strip_prefix('#') else { return Err("Hex color must begin with #".to_string()) };
    let digits = hex.encode_utf16().count();
    if digits != DIGITS {
        return Err(format!("Hex color is wrong size, expected {DIGITS} digits but got {digits}"));
    }
    if !hex.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(format!("Invalid color value: {string}"));
    }
    Ok(value)
}

fn string_color_codec<const DIGITS: usize>(components: usize) -> Pair {
    let hex = Codec::Normalize(Box::new(Codec::string()), check_hex_color::<DIGITS>);
    let numeric = Codec::Either(Box::new(types::vector(components)), Box::new(Codec::int()));
    (Codec::Either(Box::new(hex), Box::new(numeric)), Stream::Nbt)
}

struct SpawnDataList(Codec);

impl Custom for SpawnDataList {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let result = self.0.decode_partial(tag);
        match (result.value, result.error) {
            (Some(list), Some(error)) => {
                if crate::core::data_registries::is_loading() {
                    error!("Spawn data: {error}");
                }
                Ok(list)
            }
            (Some(list), None) => Ok(list),
            (None, error) => Err(error.unwrap_or_default()),
        }
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        value.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Nbt::read(r).map_err(|e| e.to_string())
    }
}

fn tri_state_value(value: Nbt) -> Result<Nbt, String> {
    Ok(match value.as_str() {
        Some("true") => Nbt::Byte(1),
        Some("false") => Nbt::Byte(0),
        _ => value,
    })
}

#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum AttributeType {
    Boolean,
    TriState,
    Float,
    AngleDegrees,
    RgbColor,
    ArgbColor,
    Integer,
    MoonPhase,
    Activity,
    BedRule,
    Particle,
    AmbientParticles,
    BackgroundMusic,
    AmbientSounds,
    MobSpawnSettings,
}

impl AttributeType {
    pub const ALL: [AttributeType; 15] = [
        AttributeType::Boolean,
        AttributeType::TriState,
        AttributeType::Float,
        AttributeType::AngleDegrees,
        AttributeType::RgbColor,
        AttributeType::ArgbColor,
        AttributeType::Integer,
        AttributeType::MoonPhase,
        AttributeType::Activity,
        AttributeType::BedRule,
        AttributeType::Particle,
        AttributeType::AmbientParticles,
        AttributeType::BackgroundMusic,
        AttributeType::AmbientSounds,
        AttributeType::MobSpawnSettings,
    ];

    pub fn name(self) -> &'static str {
        [
            "minecraft:boolean",
            "minecraft:tri_state",
            "minecraft:float",
            "minecraft:angle_degrees",
            "minecraft:rgb_color",
            "minecraft:argb_color",
            "minecraft:integer",
            "minecraft:moon_phase",
            "minecraft:activity",
            "minecraft:bed_rule",
            "minecraft:particle",
            "minecraft:ambient_particles",
            "minecraft:background_music",
            "minecraft:ambient_sounds",
            "minecraft:mob_spawn_settings",
        ][self as usize]
    }

    fn lerp(self) -> Option<LerpFunction> {
        match self {
            AttributeType::Float | AttributeType::AngleDegrees => Some(LerpFunction::Float),
            AttributeType::Integer => Some(LerpFunction::Integer),
            AttributeType::RgbColor => Some(LerpFunction::ColorVec3),
            AttributeType::ArgbColor => Some(LerpFunction::ColorVec4),
            AttributeType::AmbientParticles => Some(LerpFunction::AmbientParticleCrossFade),
            _ => None,
        }
    }

    pub fn keyframe_lerp(self) -> LerpFunction {
        self.lerp().unwrap_or(LerpFunction::Step(1.0))
    }

    pub fn state_change_lerp(self) -> LerpFunction {
        self.lerp().unwrap_or(LerpFunction::Step(0.0))
    }

    pub fn spatial_lerp(self) -> LerpFunction {
        self.lerp().unwrap_or(LerpFunction::Step(0.5))
    }

    pub fn partial_tick_lerp(self) -> LerpFunction {
        match self {
            AttributeType::AngleDegrees => LerpFunction::Degrees(90.0),
            _ => self.lerp().unwrap_or(LerpFunction::Step(0.0)),
        }
    }

    pub fn modifier(self, operation: OperationId) -> Option<AttributeModifier> {
        use OperationId as Op;
        if operation == Op::Override {
            return Some(AttributeModifier::Override);
        }
        let modifier = match (self, operation) {
            (AttributeType::Boolean, Op::And) => AttributeModifier::Boolean(BooleanModifier::And),
            (AttributeType::Boolean, Op::Nand) => AttributeModifier::Boolean(BooleanModifier::Nand),
            (AttributeType::Boolean, Op::Or) => AttributeModifier::Boolean(BooleanModifier::Or),
            (AttributeType::Boolean, Op::Nor) => AttributeModifier::Boolean(BooleanModifier::Nor),
            (AttributeType::Boolean, Op::Xor) => AttributeModifier::Boolean(BooleanModifier::Xor),
            (AttributeType::Boolean, Op::Xnor) => AttributeModifier::Boolean(BooleanModifier::Xnor),
            (AttributeType::Float | AttributeType::AngleDegrees, op) => AttributeModifier::Float(match op {
                Op::AlphaBlend => FloatModifier::AlphaBlend,
                Op::Add => FloatModifier::Add,
                Op::Subtract => FloatModifier::Subtract,
                Op::Multiply => FloatModifier::Multiply,
                Op::Minimum => FloatModifier::Minimum,
                Op::Maximum => FloatModifier::Maximum,
                _ => return None,
            }),
            (AttributeType::RgbColor, op) => AttributeModifier::Color(match op {
                Op::AlphaBlend => ColorModifier::AlphaBlendRgb,
                Op::Add => ColorModifier::AddRgb,
                Op::Subtract => ColorModifier::SubtractRgb,
                Op::Multiply => ColorModifier::MultiplyRgb,
                Op::BlendToGray => ColorModifier::BlendToGrayRgb,
                _ => return None,
            }),
            (AttributeType::ArgbColor, op) => AttributeModifier::Color(match op {
                Op::AlphaBlend => ColorModifier::AlphaBlendArgb,
                Op::Add => ColorModifier::AddArgb,
                Op::Subtract => ColorModifier::SubtractArgb,
                Op::Multiply => ColorModifier::MultiplyArgb,
                Op::BlendToGray => ColorModifier::BlendToGrayArgb,
                _ => return None,
            }),
            (AttributeType::Integer, op) => AttributeModifier::Integer(match op {
                Op::Add => IntegerModifier::Add,
                Op::Subtract => IntegerModifier::Subtract,
                Op::Multiply => IntegerModifier::Multiply,
                Op::Minimum => IntegerModifier::Minimum,
                Op::Maximum => IntegerModifier::Maximum,
                _ => return None,
            }),
            (AttributeType::AmbientParticles, Op::Append) => AttributeModifier::Append,
            (AttributeType::MobSpawnSettings, Op::Overlay) => AttributeModifier::Overlay,
            _ => return None,
        };
        Some(modifier)
    }

    pub fn parse_value(self, json: &Json) -> Result<Value, String> {
        Ok(match self {
            AttributeType::Boolean => Value::Boolean(super::parse_bool(json)?),
            AttributeType::TriState => Value::TriState(TriState::parse(json)?),
            AttributeType::Float | AttributeType::AngleDegrees => Value::Float(float(json)?),
            AttributeType::RgbColor => Value::Rgb(parse_rgb(json)?),
            AttributeType::ArgbColor => Value::Argb(parse_argb(json)?),
            AttributeType::Integer => Value::Integer(super::parse_int(json)?),
            AttributeType::MoonPhase => Value::MoonPhase(json.as_str().and_then(MoonPhase::by_name).ok_or_else(|| format!("unknown moon phase {json}"))?),
            AttributeType::Activity => {
                let name = json.as_str().ok_or_else(|| format!("not an activity: {json}"))?;
                Value::Activity(Activity::by_name(&resource_name(name)).ok_or_else(|| format!("unknown activity {name}"))?)
            }
            AttributeType::BedRule => Value::BedRule(Arc::new(BedRule::parse(json)?)),
            AttributeType::Particle | AttributeType::BackgroundMusic | AttributeType::AmbientSounds => Value::Data(Arc::new(json.clone())),
            AttributeType::AmbientParticles => {
                let list = json.as_array().ok_or_else(|| format!("not a list: {json}"))?;
                Value::AmbientParticles(Arc::new(list.iter().map(AmbientParticle::parse).collect::<Result<_, _>>()?))
            }
            AttributeType::MobSpawnSettings => Value::MobSpawnSettings(Arc::new(MobSpawnSettings::parse(json)?)),
        })
    }

    pub fn encode_value(self, value: &Value) -> Result<Nbt, String> {
        let data = |codec: Pair, json: &Json| codec.0.decode(&json_to_nbt(json));
        Ok(match (self, value) {
            (AttributeType::Boolean, Value::Boolean(value)) => Nbt::Byte(*value as i8),
            (AttributeType::Float | AttributeType::AngleDegrees, Value::Float(value)) => Nbt::Float(*value),
            (AttributeType::RgbColor, Value::Rgb(value)) => Nbt::String(hex_color(argb::transparent(argb::color_from_vector3f(*value)), 6)),
            (AttributeType::ArgbColor, Value::Argb(value)) => Nbt::String(hex_color(argb::color_from_vector4f(*value), 8)),
            (AttributeType::MoonPhase, Value::MoonPhase(value)) => Nbt::String(value.name().into()),
            (AttributeType::Particle, Value::Data(json)) => data(particle_types::dispatch_codec(), json)?,
            (AttributeType::BackgroundMusic, Value::Data(json)) => data(background_music::codec(), json)?,
            (AttributeType::AmbientSounds, Value::Data(json)) => data(ambient_sounds::codec(), json)?,
            (AttributeType::AmbientParticles, Value::AmbientParticles(particles)) => Nbt::List(
                particles
                    .iter()
                    .map(|particle| {
                        Ok(Nbt::Compound(vec![
                            ("particle".into(), data(particle_types::dispatch_codec(), &particle.particle)?),
                            ("probability".into(), Nbt::Float(particle.probability)),
                        ]))
                    })
                    .collect::<Result<_, String>>()?,
            ),
            (ty, value) => return Err(format!("{ty:?} is not synchronized with {value:?}")),
        })
    }

    pub fn condition_value_codec(self) -> Pair {
        match self {
            AttributeType::Boolean => types::bool(),
            AttributeType::Particle => particle_types::dispatch_codec(),
            AttributeType::BackgroundMusic => background_music::codec(),
            AttributeType::AmbientSounds => ambient_sounds::codec(),
            AttributeType::AmbientParticles => types::list(types::record(vec![
                types::req("particle", particle_types::dispatch_codec()),
                types::req("probability", types::float(types::codec_float_range(0.0, 1.0))),
            ])),
            AttributeType::Float | AttributeType::AngleDegrees => (Codec::float(), Stream::Float),
            AttributeType::MoonPhase => types::enumeration(&["full_moon", "waning_gibbous", "third_quarter", "waning_crescent", "new_moon", "waxing_crescent", "first_quarter", "waxing_gibbous"]),
            AttributeType::RgbColor => string_color_codec::<6>(3),
            AttributeType::ArgbColor => string_color_codec::<8>(4),
            AttributeType::TriState => (Codec::Normalize(Box::new(Codec::Either(Box::new(Codec::Bool), Box::new(Codec::Enum(&["true", "false", "default"])))), tri_state_value), Stream::Nbt),
            AttributeType::Integer => (Codec::int(), Stream::Int),
            AttributeType::Activity => types::by_name("minecraft:activity"),
            AttributeType::BedRule => types::record(vec![
                types::req("can_sleep", types::enumeration(&["always", "when_dark", "never"])),
                types::req("can_set_spawn", types::enumeration(&["always", "when_dark", "never"])),
                types::def("destroy_on_use", types::bool(), types::b(false)),
                types::def("destroy_on_leave", types::bool(), types::b(false)),
                types::opt("error_message", text::text()),
            ]),
            AttributeType::MobSpawnSettings => {
                let spawner = types::record(vec![
                    types::req("type", types::by_name("minecraft:entity_type")),
                    types::req("count", int_provider_codec()),
                    types::req("weight", types::var_int(types::non_negative_int())),
                ]);
                let cost = types::record(vec![types::req("energy_budget", (Codec::Double, Stream::Double)), types::req("charge", (Codec::Double, Stream::Double))]);
                types::record(vec![
                    types::req(
                        "spawns_by_category",
                        (Codec::Map(Box::new(Codec::Enum(&MOB_CATEGORIES)), Box::new(Codec::Custom(Box::new(SpawnDataList(Codec::list(spawner.0)))))), Stream::Nbt),
                    ),
                    types::req("spawn_costs", (Codec::Map(Box::new(types::by_name("minecraft:entity_type").0), Box::new(cost.0)), Stream::Nbt)),
                ])
            }
        }
    }

    pub fn to_float(self, value: &Value) -> Option<f32> {
        match (self, value) {
            (AttributeType::Float | AttributeType::AngleDegrees, Value::Float(f)) => Some(*f),
            (AttributeType::Integer, Value::Integer(i)) => Some(*i as f32),
            _ => None,
        }
    }

    pub fn to_int(self, value: &Value) -> Option<i32> {
        match (self, value) {
            (AttributeType::Integer, Value::Integer(i)) => Some(*i),
            _ => None,
        }
    }
}
