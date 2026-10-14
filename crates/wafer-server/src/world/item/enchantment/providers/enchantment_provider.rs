use wafer_core::random::RandomSource;

use super::enchantments_by_cost_with_difficulty::EnchantmentsByCostWithDifficulty;
use crate::world::difficulty_instance::DifficultyInstance;
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemEnchantments;

pub enum EnchantmentProvider {
    ByCostWithDifficulty(EnchantmentsByCostWithDifficulty),
}

impl EnchantmentProvider {
    pub fn enchant(&self, item: &ItemStack, item_enchantments: &mut ItemEnchantments, random: &mut impl RandomSource, difficulty: &DifficultyInstance) {
        match self {
            EnchantmentProvider::ByCostWithDifficulty(provider) => provider.enchant(item, item_enchantments, random, difficulty),
        }
    }
}
