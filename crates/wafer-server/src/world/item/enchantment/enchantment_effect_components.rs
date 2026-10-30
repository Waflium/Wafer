use std::sync::LazyLock;

use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Reader, Writer};

use super::conditional_effect::{self, ConditionalEffect};
use super::effects::enchantment_attribute_effect::{self as attribute_effect, EnchantmentAttributeEffect};
use super::effects::enchantment_entity_effect::{self as entity_effect, EnchantmentEntityEffect};
use super::effects::enchantment_location_based_effect::{self as location_based_effect, EnchantmentLocationBasedEffect};
use super::effects::enchantment_value_effect::{self as value_effect, EnchantmentValueEffect};
use super::targeted_conditional_effect::{self, TargetedConditionalEffect};
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::util::hash_ops::{self, HashCode};
use crate::world::item::component::codec::{Codec, Custom, Stream, Wrap};
use crate::world::item::component::types::{self, Pair, opt};
use crate::world::level::storage::loot::parameters::loot_context_param_sets::{self, ContextKeySet};
use crate::world::level::storage::loot::validatable::{self, Validatable};

pub const REPAIR_WITH_XP: &str = "minecraft:repair_with_xp";

#[allow(dead_code)]
#[derive(Default)]
pub struct EnchantmentEffectComponents {
    pub components: Vec<String>,
    pub damage_protection: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub damage_immunity: Vec<ConditionalEffect<()>>,
    pub damage: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub smash_damage_per_fallen_block: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub knockback: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub armor_effectiveness: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub post_attack: Vec<TargetedConditionalEffect<EnchantmentEntityEffect>>,
    pub post_piercing_attack: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
    pub hit_block: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
    pub item_damage: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub equipment_drops: Vec<TargetedConditionalEffect<EnchantmentValueEffect>>,
    pub location_changed: Vec<ConditionalEffect<EnchantmentLocationBasedEffect>>,
    pub tick: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
    pub ammo_use: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub projectile_piercing: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub projectile_spawned: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
    pub projectile_spread: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub projectile_count: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub trident_return_acceleration: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub fishing_time_reduction: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub fishing_luck_bonus: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub block_experience: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub mob_experience: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub repair_with_xp: Vec<ConditionalEffect<EnchantmentValueEffect>>,
    pub attributes: Vec<EnchantmentAttributeEffect>,
    pub crossbow_charge_time: Option<EnchantmentValueEffect>,
    pub crossbow_charging_sounds: Vec<Nbt>,
    pub trident_sound: Vec<Nbt>,
    pub prevent_equipment_drop: bool,
    pub prevent_armor_change: bool,
    pub trident_spin_attack_strength: Option<EnchantmentValueEffect>,
}

const COMPONENT_TYPES: [&str; 31] = [
    "minecraft:damage_protection",
    "minecraft:damage_immunity",
    "minecraft:damage",
    "minecraft:smash_damage_per_fallen_block",
    "minecraft:knockback",
    "minecraft:armor_effectiveness",
    "minecraft:post_attack",
    "minecraft:post_piercing_attack",
    "minecraft:hit_block",
    "minecraft:item_damage",
    "minecraft:equipment_drops",
    "minecraft:location_changed",
    "minecraft:tick",
    "minecraft:ammo_use",
    "minecraft:projectile_piercing",
    "minecraft:projectile_spawned",
    "minecraft:projectile_spread",
    "minecraft:projectile_count",
    "minecraft:trident_return_acceleration",
    "minecraft:fishing_time_reduction",
    "minecraft:fishing_luck_bonus",
    "minecraft:block_experience",
    "minecraft:mob_experience",
    "minecraft:repair_with_xp",
    "minecraft:attributes",
    "minecraft:crossbow_charge_time",
    "minecraft:crossbow_charging_sounds",
    "minecraft:trident_sound",
    "minecraft:prevent_equipment_drop",
    "minecraft:prevent_armor_change",
    "minecraft:trident_spin_attack_strength",
];

fn component_codec(component: &str) -> Pair {
    let value = || value_effect::codec();
    let conditional = |effect: Pair| types::list(conditional_effect::codec(effect));
    let targeted = |effect: Pair| types::list(targeted_conditional_effect::codec(effect));
    match component {
        "minecraft:damage_protection"
        | "minecraft:damage"
        | "minecraft:smash_damage_per_fallen_block"
        | "minecraft:knockback"
        | "minecraft:armor_effectiveness"
        | "minecraft:item_damage"
        | "minecraft:ammo_use"
        | "minecraft:projectile_piercing"
        | "minecraft:projectile_spread"
        | "minecraft:projectile_count"
        | "minecraft:trident_return_acceleration"
        | "minecraft:fishing_time_reduction"
        | "minecraft:fishing_luck_bonus"
        | "minecraft:block_experience"
        | "minecraft:mob_experience"
        | "minecraft:repair_with_xp" => conditional(value()),
        "minecraft:damage_immunity" => conditional(types::record(Vec::new())),
        "minecraft:post_attack" => targeted(entity_effect::codec()),
        "minecraft:post_piercing_attack" | "minecraft:hit_block" | "minecraft:tick" | "minecraft:projectile_spawned" => conditional(entity_effect::codec()),
        "minecraft:equipment_drops" => types::list(targeted_conditional_effect::equipment_drops_codec(value())),
        "minecraft:location_changed" => conditional(location_based_effect::codec()),
        "minecraft:attributes" => types::list(attribute_effect::map_codec()),
        "minecraft:crossbow_charge_time" | "minecraft:trident_spin_attack_strength" => value(),
        "minecraft:crossbow_charging_sounds" => {
            types::list(types::record(vec![opt("start", types::sound_event()), opt("mid", types::sound_event()), opt("end", types::sound_event())]))
        }
        "minecraft:trident_sound" => types::list(types::sound_event()),
        "minecraft:prevent_equipment_drop" | "minecraft:prevent_armor_change" => types::record(Vec::new()),
        other => unreachable!("enchantment effect component type {other}"),
    }
}

struct ComponentCodec(&'static str);

impl Custom for ComponentCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        self.decode_partial(tag).into_result()
    }

    fn decode_partial(&self, tag: &Nbt) -> DecodeResult<Nbt> {
        let typed = EnchantmentEffectComponents::default().decode_component(self.0, tag);
        if typed.error.is_some() {
            return DecodeResult { value: typed.value.map(|_| tag.clone()), error: typed.error };
        }
        component_codec(self.0).0.decode_partial(tag)
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

static COMPONENT_CODECS: LazyLock<Vec<Codec>> = LazyLock::new(|| COMPONENT_TYPES.iter().map(|&component| Codec::Custom(Box::new(ComponentCodec(component)))).collect());

fn component_entry_codec(key: &Nbt) -> Option<&'static Codec> {
    let id = key.as_str()?;
    COMPONENT_TYPES.iter().position(|&component| component == id).map(|index| &COMPONENT_CODECS[index])
}

fn component_key(tag: &Nbt) -> DecodeResult<Nbt> {
    match codec::string(tag) {
        Ok(key) => DecodeResult::from(decode_component_type(key).map(Nbt::String)),
        Err(error) => DecodeResult::error(error),
    }
}

pub fn components_codec() -> Pair {
    (Codec::Framed(Box::new(Codec::DispatchedMap(Box::new(Codec::Func(component_key)), component_entry_codec)), Wrap::Xmap), Stream::Nbt)
}

fn decode_component_type(key: &str) -> DataResult<String> {
    let id = codec::identifier(&Nbt::String(key.to_string()))?;
    if COMPONENT_TYPES.contains(&id.as_str()) {
        Ok(id)
    } else {
        Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_effect_component_type]: {id}"))
    }
}

fn value_effects(tag: &Nbt) -> DecodeResult<Vec<ConditionalEffect<EnchantmentValueEffect>>> {
    codec::list_partial(tag, |t| ConditionalEffect::decode(t, EnchantmentValueEffect::decode_partial))
}

fn entity_effects(tag: &Nbt) -> DecodeResult<Vec<ConditionalEffect<EnchantmentEntityEffect>>> {
    codec::list_partial(tag, |t| ConditionalEffect::decode(t, EnchantmentEntityEffect::decode_partial))
}

fn validated<T: Validatable>(list: DecodeResult<Vec<T>>, params: &'static ContextKeySet) -> DecodeResult<Vec<T>> {
    list.validate(|list| validatable::list_validator_for_context(params, list))
}

fn assign<T>(slot: &mut T, result: DecodeResult<T>) -> DecodeResult<()> {
    let DecodeResult { value, error } = result;
    DecodeResult {
        value: value.map(|value| {
            *slot = value;
        }),
        error,
    }
}

impl EnchantmentEffectComponents {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentEffectComponents> {
        let mut effects = EnchantmentEffectComponents::default();
        codec::wrapped(Wrap::Xmap, || codec::dispatched_map(tag, decode_component_type, |id, value| effects.decode_component(id, value).into_result()))?;
        Ok(effects)
    }

    fn decode_component(&mut self, id: &str, value: &Nbt) -> DecodeResult<()> {
        let result = match id {
            "minecraft:attributes"
            | "minecraft:crossbow_charge_time"
            | "minecraft:crossbow_charging_sounds"
            | "minecraft:trident_sound"
            | "minecraft:prevent_equipment_drop"
            | "minecraft:prevent_armor_change"
            | "minecraft:trident_spin_attack_strength" => self.decode_component_value(id, value),
            _ => codec::wrapped(Wrap::FlatXmap, || self.decode_component_value(id, value)),
        };
        if result.value.is_some() {
            self.components.push(id.to_string());
        }
        result
    }

    fn decode_component_value(&mut self, id: &str, value: &Nbt) -> DecodeResult<()> {
        let enchanted_damage = &loot_context_param_sets::ENCHANTED_DAMAGE;
        let enchanted_item = &loot_context_param_sets::ENCHANTED_ITEM;
        let enchanted_entity = &loot_context_param_sets::ENCHANTED_ENTITY;
        let result = match id {
            "minecraft:damage_protection" => assign(&mut self.damage_protection, validated(value_effects(value), enchanted_damage)),
            "minecraft:damage_immunity" => assign(
                &mut self.damage_immunity,
                validated(codec::list_partial(value, |t| ConditionalEffect::decode(t, |e| codec::map(e).map(|_| ()))), enchanted_damage),
            ),
            "minecraft:damage" => assign(&mut self.damage, validated(value_effects(value), enchanted_damage)),
            "minecraft:smash_damage_per_fallen_block" => assign(&mut self.smash_damage_per_fallen_block, validated(value_effects(value), enchanted_damage)),
            "minecraft:knockback" => assign(&mut self.knockback, validated(value_effects(value), enchanted_damage)),
            "minecraft:armor_effectiveness" => assign(&mut self.armor_effectiveness, validated(value_effects(value), enchanted_damage)),
            "minecraft:post_attack" => assign(
                &mut self.post_attack,
                validated(codec::list_partial(value, |t| TargetedConditionalEffect::decode(t, EnchantmentEntityEffect::decode_partial)), enchanted_damage),
            ),
            "minecraft:post_piercing_attack" => assign(&mut self.post_piercing_attack, validated(entity_effects(value), enchanted_damage)),
            "minecraft:hit_block" => assign(&mut self.hit_block, validated(entity_effects(value), &loot_context_param_sets::HIT_BLOCK)),
            "minecraft:item_damage" => assign(&mut self.item_damage, validated(value_effects(value), enchanted_item)),
            "minecraft:equipment_drops" => assign(
                &mut self.equipment_drops,
                validated(codec::list_partial(value, |t| TargetedConditionalEffect::decode_equipment_drops(t, EnchantmentValueEffect::decode_partial)), enchanted_damage),
            ),
            "minecraft:location_changed" => assign(
                &mut self.location_changed,
                validated(
                    codec::list_partial(value, |t| ConditionalEffect::decode(t, EnchantmentLocationBasedEffect::decode_partial)),
                    &loot_context_param_sets::ENCHANTED_LOCATION,
                ),
            ),
            "minecraft:tick" => assign(&mut self.tick, validated(entity_effects(value), enchanted_entity)),
            "minecraft:ammo_use" => assign(&mut self.ammo_use, validated(value_effects(value), enchanted_item)),
            "minecraft:projectile_piercing" => assign(&mut self.projectile_piercing, validated(value_effects(value), enchanted_item)),
            "minecraft:projectile_spawned" => assign(&mut self.projectile_spawned, validated(entity_effects(value), enchanted_entity)),
            "minecraft:projectile_spread" => assign(&mut self.projectile_spread, validated(value_effects(value), enchanted_entity)),
            "minecraft:projectile_count" => assign(&mut self.projectile_count, validated(value_effects(value), enchanted_entity)),
            "minecraft:trident_return_acceleration" => assign(&mut self.trident_return_acceleration, validated(value_effects(value), enchanted_entity)),
            "minecraft:fishing_time_reduction" => assign(&mut self.fishing_time_reduction, validated(value_effects(value), enchanted_entity)),
            "minecraft:fishing_luck_bonus" => assign(&mut self.fishing_luck_bonus, validated(value_effects(value), enchanted_entity)),
            "minecraft:block_experience" => assign(&mut self.block_experience, validated(value_effects(value), enchanted_item)),
            "minecraft:mob_experience" => assign(&mut self.mob_experience, validated(value_effects(value), enchanted_entity)),
            "minecraft:repair_with_xp" => assign(&mut self.repair_with_xp, validated(value_effects(value), enchanted_item)),
            "minecraft:attributes" => assign(&mut self.attributes, codec::list_partial(value, EnchantmentAttributeEffect::decode_partial)),
            "minecraft:crossbow_charge_time" => assign(&mut self.crossbow_charge_time, EnchantmentValueEffect::decode_partial(value).map(Some)),
            "minecraft:crossbow_charging_sounds" => assign(&mut self.crossbow_charging_sounds, codec::checked(&component_codec(id).0, value, codec::list)),
            "minecraft:trident_sound" => assign(&mut self.trident_sound, codec::checked(&component_codec(id).0, value, codec::list)),
            "minecraft:prevent_equipment_drop" => assign(&mut self.prevent_equipment_drop, codec::checked(&component_codec(id).0, value, |_| Ok(true))),
            "minecraft:prevent_armor_change" => assign(&mut self.prevent_armor_change, codec::checked(&component_codec(id).0, value, |_| Ok(true))),
            "minecraft:trident_spin_attack_strength" => assign(&mut self.trident_spin_attack_strength, EnchantmentValueEffect::decode_partial(value).map(Some)),
            other => unreachable!("enchantment effect component type {other}"),
        };
        result
    }

    pub fn has(&self, component: &str) -> bool {
        self.components.iter().any(|c| c == component)
    }
}
