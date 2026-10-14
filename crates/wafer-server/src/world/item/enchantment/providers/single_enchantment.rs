use wafer_core::random::RandomSource;

use crate::util::valueproviders::IntProvider;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::enchantment::enchantment::Enchantment;

pub struct SingleEnchantment {
    pub enchantment: &'static str,
    pub level: IntProvider,
}

impl SingleEnchantment {
    pub fn enchant(&self, item_enchantments: &mut ItemEnchantments, random: &mut impl RandomSource) {
        let enchantment = Enchantment::by_id(self.enchantment).expect("a built-in enchantment");
        item_enchantments.upgrade(self.enchantment, self.level.sample(random).clamp(enchantment.min_level(), enchantment.max_level()));
    }
}
