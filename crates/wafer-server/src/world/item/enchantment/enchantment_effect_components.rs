use wafer_protocol::nbt::Nbt;

use super::conditional_effect::ConditionalEffect;
use super::effects::enchantment_attribute_effect::EnchantmentAttributeEffect;
use super::effects::enchantment_entity_effect::EnchantmentEntityEffect;
use super::effects::enchantment_location_based_effect::EnchantmentLocationBasedEffect;
use super::effects::enchantment_value_effect::EnchantmentValueEffect;
use super::targeted_conditional_effect::TargetedConditionalEffect;
use crate::util::codec::{self, DataResult};
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

fn value_effects(tag: &Nbt) -> DataResult<Vec<ConditionalEffect<EnchantmentValueEffect>>> {
    codec::list_of(tag, |t| ConditionalEffect::decode(t, EnchantmentValueEffect::decode))
}

fn entity_effects(tag: &Nbt) -> DataResult<Vec<ConditionalEffect<EnchantmentEntityEffect>>> {
    codec::list_of(tag, |t| ConditionalEffect::decode(t, EnchantmentEntityEffect::decode))
}

fn validated<T: Validatable>(list: Vec<T>, params: &'static ContextKeySet) -> DataResult<Vec<T>> {
    validatable::list_validator_for_context(params, &list)?;
    Ok(list)
}

impl EnchantmentEffectComponents {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentEffectComponents> {
        let mut effects = EnchantmentEffectComponents::default();
        let mut errors = Vec::new();
        let mut failed = Vec::new();
        for (key, value) in codec::map(tag)? {
            if let Err(error) = codec::identifier(&Nbt::String(key.clone())).and_then(|id| effects.decode_component(id, value)) {
                errors.push(error);
                failed.push((key.clone(), value.clone()));
            }
        }
        if !errors.is_empty() {
            return Err(format!("{} missed input: {}", errors.join("; "), crate::nbt::to_string(&Nbt::Compound(failed))));
        }
        Ok(effects)
    }

    fn decode_component(&mut self, id: String, value: &Nbt) -> DataResult<()> {
        match id.as_str() {
            "minecraft:damage_protection" => self.damage_protection = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_DAMAGE)?,
            "minecraft:damage_immunity" => {
                self.damage_immunity = validated(codec::list_of(value, |t| ConditionalEffect::decode(t, |e| codec::map(e).map(|_| ())))?, &loot_context_param_sets::ENCHANTED_DAMAGE)?;
            }
            "minecraft:damage" => self.damage = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_DAMAGE)?,
            "minecraft:smash_damage_per_fallen_block" => self.smash_damage_per_fallen_block = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_DAMAGE)?,
            "minecraft:knockback" => self.knockback = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_DAMAGE)?,
            "minecraft:armor_effectiveness" => self.armor_effectiveness = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_DAMAGE)?,
            "minecraft:post_attack" => {
                self.post_attack = validated(codec::list_of(value, |t| TargetedConditionalEffect::decode(t, EnchantmentEntityEffect::decode))?, &loot_context_param_sets::ENCHANTED_DAMAGE)?;
            }
            "minecraft:post_piercing_attack" => self.post_piercing_attack = validated(entity_effects(value)?, &loot_context_param_sets::ENCHANTED_DAMAGE)?,
            "minecraft:hit_block" => self.hit_block = validated(entity_effects(value)?, &loot_context_param_sets::HIT_BLOCK)?,
            "minecraft:item_damage" => self.item_damage = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ITEM)?,
            "minecraft:equipment_drops" => {
                self.equipment_drops = validated(codec::list_of(value, |t| TargetedConditionalEffect::decode_equipment_drops(t, EnchantmentValueEffect::decode))?, &loot_context_param_sets::ENCHANTED_DAMAGE)?;
            }
            "minecraft:location_changed" => {
                self.location_changed = validated(codec::list_of(value, |t| ConditionalEffect::decode(t, EnchantmentLocationBasedEffect::decode))?, &loot_context_param_sets::ENCHANTED_LOCATION)?;
            }
            "minecraft:tick" => self.tick = validated(entity_effects(value)?, &loot_context_param_sets::ENCHANTED_ENTITY)?,
            "minecraft:ammo_use" => self.ammo_use = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ITEM)?,
            "minecraft:projectile_piercing" => self.projectile_piercing = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ITEM)?,
            "minecraft:projectile_spawned" => self.projectile_spawned = validated(entity_effects(value)?, &loot_context_param_sets::ENCHANTED_ENTITY)?,
            "minecraft:projectile_spread" => self.projectile_spread = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ENTITY)?,
            "minecraft:projectile_count" => self.projectile_count = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ENTITY)?,
            "minecraft:trident_return_acceleration" => self.trident_return_acceleration = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ENTITY)?,
            "minecraft:fishing_time_reduction" => self.fishing_time_reduction = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ENTITY)?,
            "minecraft:fishing_luck_bonus" => self.fishing_luck_bonus = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ENTITY)?,
            "minecraft:block_experience" => self.block_experience = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ITEM)?,
            "minecraft:mob_experience" => self.mob_experience = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ENTITY)?,
            "minecraft:repair_with_xp" => self.repair_with_xp = validated(value_effects(value)?, &loot_context_param_sets::ENCHANTED_ITEM)?,
            "minecraft:attributes" => self.attributes = codec::list_of(value, EnchantmentAttributeEffect::decode)?,
            "minecraft:crossbow_charge_time" => self.crossbow_charge_time = Some(EnchantmentValueEffect::decode(value)?),
            "minecraft:crossbow_charging_sounds" => self.crossbow_charging_sounds = codec::list(value)?,
            "minecraft:trident_sound" => self.trident_sound = codec::list(value)?,
            "minecraft:prevent_equipment_drop" => self.prevent_equipment_drop = true,
            "minecraft:prevent_armor_change" => self.prevent_armor_change = true,
            "minecraft:trident_spin_attack_strength" => self.trident_spin_attack_strength = Some(EnchantmentValueEffect::decode(value)?),
            other => return Err(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_effect_component_type]: {other}")),
        }
        self.components.push(id);
        Ok(())
    }

    pub fn has(&self, component: &str) -> bool {
        self.components.iter().any(|c| c == component)
    }
}
