use wafer_core::mth;
use wafer_core::random::RandomSource;

use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::item::ItemStack;
use crate::world::item::component::codec;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::enchantment::enchantment_helper;

pub struct EnchantmentsByCostWithDifficulty {
    pub enchantments: &'static str,
    pub min_cost: i32,
    pub max_cost_span: i32,
}

impl EnchantmentsByCostWithDifficulty {
    pub fn enchant(&self, item: &ItemStack, item_enchantments: &mut ItemEnchantments, random: &mut impl RandomSource, difficulty: &DifficultyInstance) {
        let difficulty_modifier = difficulty.get_special_multiplier();
        let cost = mth::random_between_inclusive(random, self.min_cost, self.min_cost + (difficulty_modifier * self.max_cost_span as f32) as i32);
        let names = codec::registry("minecraft:enchantment");
        let source: Vec<&str> = wafer_data::tag("minecraft:enchantment", self.enchantments).iter().map(|&id| names[id as usize]).collect();
        for instance in enchantment_helper::select_enchantment(random, item, cost, &source) {
            item_enchantments.upgrade(&instance.enchantment, instance.level);
        }
    }
}
