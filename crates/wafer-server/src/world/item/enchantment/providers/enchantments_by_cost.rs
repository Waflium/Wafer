use serde_json::Value;
use wafer_core::random::RandomSource;

use super::enchantment_provider;
use crate::core::holder_set::HolderSet;
use crate::util::valueproviders::IntProvider;
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::enchantment::enchantment_helper;

pub struct EnchantmentsByCost {
    pub enchantments: HolderSet,
    pub cost: IntProvider,
}

impl EnchantmentsByCost {
    pub fn parse(json: &Value) -> Result<EnchantmentsByCost, String> {
        Ok(EnchantmentsByCost { enchantments: enchantment_provider::parse_enchantments(&json["enchantments"])?, cost: IntProvider::parse(&json["cost"])? })
    }

    pub fn enchant(&self, item: &ItemStack, item_enchantments: &mut ItemEnchantments, random: &mut impl RandomSource) {
        let cost = self.cost.sample(random);
        let source = enchantment_provider::enchantment_names(&self.enchantments);
        for instance in enchantment_helper::select_enchantment(random, item, cost, &source) {
            item_enchantments.upgrade(&instance.enchantment, instance.level);
        }
    }
}
