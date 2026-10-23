use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::item::enchantment::enchantment::Enchantment;
use crate::world::item::enchantment::level_based_value::LevelBasedValue;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct LootItemRandomChanceWithEnchantedBonusCondition {
    unenchanted_chance: f32,
    enchanted_chance: LevelBasedValue,
    enchantment: String,
}

impl LootItemRandomChanceWithEnchantedBonusCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<LootItemRandomChanceWithEnchantedBonusCondition> {
        let mut record = codec::Record::default();
        let unenchanted_chance = record.required(tag, "unenchanted_chance", |t| {
            let value = codec::float(t)?;
            if !(0.0..=1.0).contains(&value) {
                return Err(format!("Value {value} outside of range [0.0:1.0]"));
            }
            Ok(value)
        });
        let enchanted_chance = record.required(tag, "enchanted_chance", LevelBasedValue::decode);
        let enchantment = record.required(tag, "enchantment", Enchantment::decode_holder);
        record.build(|| {
            Some(LootItemRandomChanceWithEnchantedBonusCondition { unenchanted_chance: unenchanted_chance?, enchanted_chance: enchanted_chance?, enchantment: enchantment? })
        })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        let enchantment_level = match context.params.attacking_entity {
            Some(killer) if context.params.level.is_living(killer) => context.params.level.get_enchantment_level(&self.enchantment, killer),
            _ => 0,
        };
        let chance = if enchantment_level > 0 { self.enchanted_chance.calculate(enchantment_level) } else { self.unenchanted_chance };
        context.random().next_float() < chance
    }
}

impl LootItemRandomChanceWithEnchantedBonusCondition {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![LootContextParam::AttackingEntity]
    }
}
