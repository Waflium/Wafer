use serde_json::Value;
use wafer_core::mth;
use wafer_core::random::RandomSource;

use super::enchantment_provider;
use crate::core::holder_set::HolderSet;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::enchantment::enchantment_helper;

pub struct EnchantmentsByCostWithDifficulty {
    pub enchantments: HolderSet,
    pub min_cost: i32,
    pub max_cost_span: i32,
}

impl EnchantmentsByCostWithDifficulty {
    pub fn parse(json: &Value) -> Result<EnchantmentsByCostWithDifficulty, String> {
        Ok(EnchantmentsByCostWithDifficulty {
            enchantments: enchantment_provider::parse_enchantments(&json["enchantments"])?,
            min_cost: enchantment_provider::int_range(&json["min_cost"], 1, 10000)?,
            max_cost_span: enchantment_provider::int_range(&json["max_cost_span"], 0, 10000)?,
        })
    }

    pub fn enchant(&self, item: &ItemStack, item_enchantments: &mut ItemEnchantments, random: &mut impl RandomSource, difficulty: &DifficultyInstance) {
        let difficulty_modifier = difficulty.get_special_multiplier();
        let cost = mth::random_between_inclusive(random, self.min_cost, self.min_cost + (difficulty_modifier * self.max_cost_span as f32) as i32);
        let source = enchantment_provider::enchantment_names(&self.enchantments);
        for instance in enchantment_helper::select_enchantment(random, item, cost, &source) {
            item_enchantments.upgrade(&instance.enchantment, instance.level);
        }
    }
}
