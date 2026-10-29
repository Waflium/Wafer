use std::borrow::Cow;
use std::sync::OnceLock;

use serde_json::Value;

use crate::util::valueproviders::{FloatProvider, float_provider_codec_in};
use crate::world::entity::attributes::{Attribute, AttributeModifier, Operation};
use crate::world::item::ItemStack;
use crate::world::item::component::codec::{Codec, Stream};
use crate::world::item::component::types::{self, Pair, def, opt, record, req};
use crate::world::item::component::values::HolderSet;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct ExplosionData {
    pub power: i32,
    pub causes_fire: bool,
    pub fuse: i32,
}

#[derive(Clone, Debug)]
pub struct ContactDamage {
    pub damage_type: &'static str,
    pub amount: FloatProvider,
    pub attribute_to_source: bool,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct KnockbackModifiers {
    pub horizontal_power: f32,
    pub vertical_power: f32,
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SoundSettings {
    pub hit_sound: &'static str,
    pub push_sound: &'static str,
    pub push_sound_impulse_threshold: f32,
    pub push_sound_cooldown: f32,
}

pub const DEFAULT_KNOCKBACK_MODIFIERS: KnockbackModifiers = KnockbackModifiers { horizontal_power: 0.33, vertical_power: 0.06 };
pub const DEFAULT_SOUND_SETTINGS: SoundSettings = SoundSettings {
    hit_sound: "minecraft:entity.sulfur_cube.regular.hit",
    push_sound: "minecraft:entity.sulfur_cube.regular.push",
    push_sound_impulse_threshold: 0.2,
    push_sound_cooldown: 0.5,
};

#[derive(Debug)]
pub struct SulfurCubeArchetype {
    pub items: HolderSet,
    pub attribute_modifiers: Vec<(Attribute, AttributeModifier)>,
    pub buoyant: bool,
    pub explosion: Option<ExplosionData>,
    pub contact_damage: Option<ContactDamage>,
    pub knockback_modifiers: KnockbackModifiers,
    pub sound_settings: SoundSettings,
}

fn attribute_entry_codec() -> Pair {
    record(vec![
        req("attribute", types::by_name("minecraft:attribute")),
        req("id", types::identifier()),
        req("amount", (Codec::Double, Stream::Double)),
        req("operation", types::enumeration(&["add_value", "add_multiplied_base", "add_multiplied_total"])),
    ])
}

fn explosion_data_codec() -> Pair {
    record(vec![
        req("power", types::var_int(types::non_negative_int())),
        req("causes_fire", types::bool()),
        req("fuse", types::var_int(types::positive_int())),
    ])
}

fn contact_damage_codec() -> Pair {
    record(vec![
        req("damage_type", types::fixed("minecraft:damage_type")),
        req("amount", float_provider_codec_in::<{ 0.0f32.to_bits() }, { f32::INFINITY.to_bits() }>()),
        req("attribute_to_source", types::bool()),
    ])
}

fn knockback_modifiers_codec() -> Pair {
    record(vec![req("horizontal_power", types::float(Codec::float())), req("vertical_power", types::float(Codec::float()))])
}

fn sound_settings_codec() -> Pair {
    record(vec![
        req("hit_sound", types::sound_event()),
        req("push_sound", types::sound_event()),
        req("push_sound_impulse_threshold", types::float(Codec::float())),
        req("push_sound_cooldown", types::float(Codec::float())),
    ])
}

pub fn direct_codec() -> Pair {
    record(vec![
        req("items", types::holder_set("minecraft:item")),
        req("attribute_modifiers", types::list(attribute_entry_codec())),
        def("buoyant", types::bool(), types::b(false)),
        opt("explosion", explosion_data_codec()),
        opt("contact_damage", contact_damage_codec()),
        req("knockback_modifiers", knockback_modifiers_codec()),
        req("sound_settings", sound_settings_codec()),
    ])
}

fn leak(value: &str) -> &'static str {
    Box::leak(value.to_string().into_boxed_str())
}

fn identifier(value: &Value) -> Option<String> {
    crate::commands::arguments::parse_identifier(value.as_str()?)
}

fn attribute_entry(entry: &Value) -> Option<(Attribute, AttributeModifier)> {
    let attribute = Attribute::by_name(&identifier(entry.get("attribute")?)?)?;
    let modifier = AttributeModifier {
        id: Cow::Owned(identifier(entry.get("id")?)?),
        amount: entry.get("amount")?.as_f64()?,
        operation: Operation::by_name(entry.get("operation")?.as_str()?)?,
    };
    Some((attribute, modifier))
}

impl SulfurCubeArchetype {
    fn parse(json: &Value) -> Option<SulfurCubeArchetype> {
        let items = HolderSet::from_nbt(&crate::world::item::component::json_to_nbt(json.get("items")?));
        let attribute_modifiers = json.get("attribute_modifiers")?.as_array()?.iter().map(attribute_entry).collect::<Option<Vec<_>>>()?;
        let explosion = match json.get("explosion") {
            Some(explosion) => Some(ExplosionData {
                power: explosion.get("power")?.as_i64()? as i32,
                causes_fire: explosion.get("causes_fire")?.as_bool()?,
                fuse: explosion.get("fuse")?.as_i64()? as i32,
            }),
            None => None,
        };
        let contact_damage = match json.get("contact_damage") {
            Some(contact_damage) => Some(ContactDamage {
                damage_type: leak(&identifier(contact_damage.get("damage_type")?)?),
                amount: FloatProvider::parse(contact_damage.get("amount")?).ok()?,
                attribute_to_source: contact_damage.get("attribute_to_source")?.as_bool()?,
            }),
            None => None,
        };
        let knockback = json.get("knockback_modifiers")?;
        let sounds = json.get("sound_settings")?;
        Some(SulfurCubeArchetype {
            items,
            attribute_modifiers,
            buoyant: json.get("buoyant").and_then(Value::as_bool).unwrap_or(false),
            explosion,
            contact_damage,
            knockback_modifiers: KnockbackModifiers {
                horizontal_power: knockback.get("horizontal_power")?.as_f64()? as f32,
                vertical_power: knockback.get("vertical_power")?.as_f64()? as f32,
            },
            sound_settings: SoundSettings {
                hit_sound: leak(&identifier(sounds.get("hit_sound")?)?),
                push_sound: leak(&identifier(sounds.get("push_sound")?)?),
                push_sound_impulse_threshold: sounds.get("push_sound_impulse_threshold")?.as_f64()? as f32,
                push_sound_cooldown: sounds.get("push_sound_cooldown")?.as_f64()? as f32,
            },
        })
    }

    pub fn registered() -> &'static [SulfurCubeArchetype] {
        static ARCHETYPES: OnceLock<Vec<SulfurCubeArchetype>> = OnceLock::new();
        ARCHETYPES.get_or_init(|| {
            crate::core::data_registries::json_entries("minecraft:sulfur_cube_archetype")
                .iter()
                .map(|&(_, json)| SulfurCubeArchetype::parse(&serde_json::from_str(json).expect("vanilla JSON")).expect("vanilla sulfur cube archetype"))
                .collect()
        })
    }

    pub fn matches(&self, stack: &ItemStack) -> bool {
        let item = stack.get_item();
        self.items.contains("minecraft:item", item.name(), item.0 as i32)
    }
}
