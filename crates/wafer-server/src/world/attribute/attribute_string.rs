use wafer_protocol::nbt::Nbt;

use super::environment_attribute::EnvironmentAttributeInfo;
use super::environment_attribute_map::Entry;
use super::modifier::{AttributeModifier, BooleanModifier, ColorModifier, FloatModifier, IntegerModifier};
use super::{AttributeType, BedRule, Value};
use crate::nbt::{java_double_to_string, java_float_to_string};
use crate::util::argb;
use crate::util::java_collections::{identity_hash_code, identity_hash_code_of};
use crate::util::tri_state::TriState;
use crate::world::item::component::codec::{as_f32, as_f64, as_i32};
use crate::world::level::levelgen::density_function::resource_name;

pub fn entry_to_string(attribute: &EnvironmentAttributeInfo, entry: &Entry, argument: &Nbt) -> String {
    format!("Entry[argument={}, modifier={}]", value_to_string(attribute.ty, &entry.argument, argument), modifier_to_string(entry.modifier))
}

pub fn value_to_string(ty: AttributeType, value: &Value, tag: &Nbt) -> String {
    match value {
        Value::Boolean(value) => value.to_string(),
        Value::TriState(TriState::True) => "TRUE".to_string(),
        Value::TriState(TriState::False) => "FALSE".to_string(),
        Value::TriState(TriState::Default) => "DEFAULT".to_string(),
        Value::Float(value) => java_float_to_string(*value),
        Value::Integer(value) => value.to_string(),
        Value::Rgb(vector) => joml_vector_to_string(vector),
        Value::Argb(vector) => joml_vector_to_string(vector),
        Value::MoonPhase(phase) => phase.name().to_ascii_uppercase(),
        Value::Activity(activity) => activity.get_name().to_string(),
        Value::BedRule(rule) => bed_rule_to_string(rule, tag),
        Value::AmbientParticles(_) => ambient_particles_to_string(tag),
        Value::MobSpawnSettings(_) => format!("net.minecraft.world.level.biome.MobSpawnSettings@{:x}", identity_hash_code()),
        Value::Data(_) => match ty {
            AttributeType::Particle => particle_to_string(tag),
            AttributeType::BackgroundMusic => background_music_to_string(tag),
            _ => ambient_sounds_to_string(tag),
        },
        Value::FloatWithAlpha(argument) => format!("FloatWithAlpha[value={}, alpha={}]", java_float_to_string(argument.value), java_float_to_string(argument.alpha)),
        Value::BlendToGray(argument) => format!("BlendToGray[brightness={}, factor={}]", java_float_to_string(argument.brightness), java_float_to_string(argument.factor)),
    }
}

pub fn modifier_to_string(modifier: AttributeModifier) -> String {
    match modifier {
        AttributeModifier::Override => "OverrideModifier[]".to_string(),
        AttributeModifier::Boolean(modifier) => match modifier {
            BooleanModifier::And => "AND",
            BooleanModifier::Nand => "NAND",
            BooleanModifier::Or => "OR",
            BooleanModifier::Nor => "NOR",
            BooleanModifier::Xor => "XOR",
            BooleanModifier::Xnor => "XNOR",
        }
        .to_string(),
        AttributeModifier::Float(modifier) => match modifier {
            FloatModifier::AlphaBlend => anonymous_class("FloatModifier", 1, "FloatModifier.ALPHA_BLEND"),
            FloatModifier::Add => lambda("FloatModifier", "FloatModifier.ADD", 0x96909118),
            FloatModifier::Subtract => lambda("FloatModifier", "FloatModifier.SUBTRACT", 0x96909398),
            FloatModifier::Multiply => lambda("FloatModifier", "FloatModifier.MULTIPLY", 0x96909618),
            FloatModifier::Minimum => lambda("FloatModifier", "FloatModifier.MINIMUM", 0x96909898),
            FloatModifier::Maximum => lambda("FloatModifier", "FloatModifier.MAXIMUM", 0x96909b18),
        },
        AttributeModifier::Integer(modifier) => match modifier {
            IntegerModifier::Add => lambda("IntegerModifier", "IntegerModifier.ADD", 0x9690e208),
            IntegerModifier::Subtract => lambda("IntegerModifier", "IntegerModifier.SUBTRACT", 0x9690e4b0),
            IntegerModifier::Multiply => lambda("IntegerModifier", "IntegerModifier.MULTIPLY", 0x9690e758),
            IntegerModifier::Minimum => lambda("IntegerModifier", "IntegerModifier.MINIMUM", 0x9690ea00),
            IntegerModifier::Maximum => lambda("IntegerModifier", "IntegerModifier.MAXIMUM", 0x9690eca8),
        },
        AttributeModifier::Color(modifier) => match modifier {
            ColorModifier::AlphaBlendRgb => lambda("ColorModifier", "ColorModifier.ALPHA_BLEND_RGB", 0x9690a1a8),
            ColorModifier::AlphaBlendArgb => lambda("ColorModifier", "ColorModifier.ALPHA_BLEND_ARGB", 0x9690a428),
            ColorModifier::AddRgb => lambda("ColorModifier", "ColorModifier.ADD_RGB", 0x9690a8b0),
            ColorModifier::SubtractRgb => lambda("ColorModifier", "ColorModifier.SUBTRACT_RGB", 0x9690ab30),
            ColorModifier::MultiplyRgb => lambda("ColorModifier", "ColorModifier.MULTIPLY_RGB", 0x9690adb0),
            ColorModifier::AddArgb => lambda("ColorModifier", "ColorModifier.ADD_ARGB", 0x9690b030),
            ColorModifier::SubtractArgb => lambda("ColorModifier", "ColorModifier.SUBTRACT_ARGB", 0x9690b2b0),
            ColorModifier::MultiplyArgb => lambda("ColorModifier", "ColorModifier.MULTIPLY_ARGB", 0x9690b530),
            ColorModifier::BlendToGrayRgb => anonymous_class("ColorModifier", 1, "ColorModifier.BLEND_TO_GRAY_RGB"),
            ColorModifier::BlendToGrayArgb => anonymous_class("ColorModifier", 2, "ColorModifier.BLEND_TO_GRAY_ARGB"),
        },
        AttributeModifier::Append => "Append[]".to_string(),
        AttributeModifier::Overlay => "Overlay[]".to_string(),
    }
}

fn anonymous_class(class: &str, index: u32, object: &str) -> String {
    format!("net.minecraft.world.attribute.modifier.{class}${index}@{:x}", identity_hash_code_of(object))
}

fn lambda(class: &str, object: &str, address: u64) -> String {
    format!("net.minecraft.world.attribute.modifier.{class}$$Lambda/0x{address:016x}@{:x}", identity_hash_code_of(object))
}

pub fn joml_vector_to_string(components: &[f32]) -> String {
    let numbers: Vec<String> = components.iter().map(|&component| joml_number(component as f64)).collect();
    format!("({})", numbers.join(" "))
}

fn joml_number(number: f64) -> String {
    if number.is_nan() {
        return "      NaN".to_string();
    }
    if number.is_infinite() {
        return format!("     {}Inf", if number > 0.0 { "+" } else { "-" });
    }
    let text = format!("{number:.3e}");
    let (mantissa, exponent) = text.split_once('e').expect("a scientific number");
    let exponent: i32 = exponent.parse().expect("an exponent");
    let sign = if mantissa.starts_with('-') { "" } else { " " };
    let exponent_sign = if exponent >= 0 { "+" } else { "" };
    format!("{sign}{mantissa}E{exponent_sign}{exponent}")
}

fn optional(value: Option<String>) -> String {
    match value {
        Some(value) => format!("Optional[{value}]"),
        None => "Optional.empty".to_string(),
    }
}

fn bed_rule_to_string(rule: &BedRule, tag: &Nbt) -> String {
    let rule_name = |rule: super::bed_rule::Rule| match rule {
        super::bed_rule::Rule::Always => "ALWAYS",
        super::bed_rule::Rule::WhenDark => "WHEN_DARK",
        super::bed_rule::Rule::Never => "NEVER",
    };
    let error_message = tag.get("error_message").and_then(crate::network::chat::component_serialization::decode_or_partial).map(|message| message.java_to_string());
    format!(
        "BedRule[canSleep={}, canSetSpawn={}, destroyOnUse={}, destroyOnLeave={}, errorMessage={}]",
        rule_name(rule.can_sleep),
        rule_name(rule.can_set_spawn),
        rule.destroy_on_use,
        rule.destroy_on_leave,
        optional(error_message)
    )
}

fn sound_event_holder_to_string(tag: &Nbt) -> String {
    match tag {
        Nbt::String(id) => {
            let id = resource_name(id);
            format!("Reference{{ResourceKey[minecraft:sound_event / {id}]=SoundEvent[location={id}, fixedRange=Optional.empty]}}")
        }
        _ => {
            let location = tag.get("sound_id").and_then(Nbt::as_str).map(resource_name).unwrap_or_default();
            let range = tag.get("range").filter(|range| range.as_f64().is_some()).map(|range| java_float_to_string(as_f32(range)));
            format!("Direct{{SoundEvent[location={location}, fixedRange={}]}}", optional(range))
        }
    }
}

fn music_to_string(tag: &Nbt) -> String {
    let int = |key: &str| tag.get(key).map_or(0, as_i32);
    let replace_current_music = tag.get("replace_current_music").and_then(Nbt::as_i64).is_some_and(|value| value != 0);
    format!(
        "Music[sound={}, minDelay={}, maxDelay={}, replaceCurrentMusic={replace_current_music}]",
        tag.get("sound").map(sound_event_holder_to_string).unwrap_or_default(),
        int("min_delay"),
        int("max_delay")
    )
}

fn background_music_to_string(tag: &Nbt) -> String {
    let music = |key: &str| optional(tag.get(key).map(music_to_string));
    format!("BackgroundMusic[defaultMusic={}, creativeMusic={}, underwaterMusic={}]", music("default"), music("creative"), music("underwater"))
}

fn ambient_mood_settings_to_string(tag: &Nbt) -> String {
    let int = |key: &str| tag.get(key).map_or(0, as_i32);
    format!(
        "AmbientMoodSettings[soundEvent={}, tickDelay={}, blockSearchExtent={}, soundPositionOffset={}]",
        tag.get("sound").map(sound_event_holder_to_string).unwrap_or_default(),
        int("tick_delay"),
        int("block_search_extent"),
        java_double_to_string(tag.get("offset").map_or(0.0, as_f64))
    )
}

fn ambient_additions_settings_to_string(tag: &Nbt) -> String {
    format!(
        "AmbientAdditionsSettings[soundEvent={}, tickChance={}]",
        tag.get("sound").map(sound_event_holder_to_string).unwrap_or_default(),
        java_double_to_string(tag.get("tick_chance").map_or(0.0, as_f64))
    )
}

fn ambient_sounds_to_string(tag: &Nbt) -> String {
    let additions: Vec<String> = match tag.get("additions") {
        Some(Nbt::List(items)) => items.iter().map(ambient_additions_settings_to_string).collect(),
        Some(single) => vec![ambient_additions_settings_to_string(single)],
        None => Vec::new(),
    };
    format!(
        "AmbientSounds[loop={}, mood={}, additions=[{}]]",
        optional(tag.get("loop").map(sound_event_holder_to_string)),
        optional(tag.get("mood").map(ambient_mood_settings_to_string)),
        additions.join(", ")
    )
}

fn ambient_particles_to_string(tag: &Nbt) -> String {
    let particles: Vec<String> = tag
        .as_list()
        .iter()
        .map(|particle| {
            format!(
                "AmbientParticle[particle={}, probability={}]",
                particle.get("particle").map(particle_to_string).unwrap_or_default(),
                java_float_to_string(particle.get("probability").map_or(0.0, as_f32))
            )
        })
        .collect();
    format!("[{}]", particles.join(", "))
}

fn particle_class(particle_type: &str) -> Option<&'static str> {
    Some(match particle_type {
        "minecraft:block" | "minecraft:block_marker" | "minecraft:falling_dust" | "minecraft:dust_pillar" | "minecraft:block_crumble" => "BlockParticleOption",
        "minecraft:dragon_breath" => "PowerParticleOption",
        "minecraft:dust" => "DustParticleOptions",
        "minecraft:dust_color_transition" => "DustColorTransitionOptions",
        "minecraft:effect" | "minecraft:instant_effect" => "SpellParticleOption",
        "minecraft:entity_effect" | "minecraft:tinted_leaves" | "minecraft:flash" => "ColorParticleOption",
        "minecraft:item" => "ItemParticleOption",
        "minecraft:vibration" => "VibrationParticleOption",
        "minecraft:shriek" => "ShriekParticleOption",
        _ => return None,
    })
}

fn particle_to_string(tag: &Nbt) -> String {
    let particle_type = tag.get("type").and_then(Nbt::as_str).map(resource_name).unwrap_or_default();
    let int = |key: &str| tag.get(key).map_or(0, as_i32);
    let float = |key: &str| java_float_to_string(tag.get(key).map_or(0.0, as_f32));
    let particle_type_object = || format!("net.minecraft.core.particles.ParticleTypes$1@{:x}", identity_hash_code_of(&format!("particle_type:{particle_type}")));
    match particle_type.as_str() {
        "minecraft:trail" => {
            let target = tag.get("target").map(|target| target.as_list().iter().map(|component| java_double_to_string(as_f64(component))).collect::<Vec<_>>().join(", ")).unwrap_or_default();
            let color = match tag.get("color") {
                Some(Nbt::List(components)) => {
                    let component = |index: usize| components.get(index).map_or(0.0, as_f32);
                    argb::color_from_float(1.0, component(0), component(1), component(2))
                }
                Some(color) => as_i32(color),
                None => 0,
            };
            format!("TrailParticleOption[target=({target}), color={color}, duration={}]", int("duration"))
        }
        "minecraft:sculk_charge" => format!("SculkChargeParticleOptions[roll={}]", float("roll")),
        "minecraft:geyser" | "minecraft:geyser_plume" => format!("GeyserParticleOptions[type={}, waterBlocks={}]", particle_type_object(), int("water_blocks")),
        "minecraft:geyser_base" | "minecraft:geyser_poof" => {
            format!("GeyserBaseParticleOptions[type={}, waterBlocks={}, burstImpulseBase={}]", particle_type_object(), int("water_blocks"), float("burst_impulse_base"))
        }
        other => match particle_class(other) {
            Some(class) => format!("net.minecraft.core.particles.{class}@{:x}", identity_hash_code()),
            None => format!("net.minecraft.core.particles.SimpleParticleType@{:x}", identity_hash_code_of(&format!("particle:{other}"))),
        },
    }
}
