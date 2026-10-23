use serde_json::Value;
use wafer_core::random::RandomSource;

use crate::util::valueproviders::IntProvider;
use crate::world::item::component::codec;
use crate::world::item::component::values::ItemEnchantments;
use crate::world::item::enchantment::enchantment::Enchantment;

pub struct SingleEnchantment {
    pub enchantment: &'static str,
    pub level: IntProvider,
}

impl SingleEnchantment {
    pub fn parse(json: &Value) -> Result<SingleEnchantment, String> {
        let name = json["enchantment"].as_str().ok_or("No key enchantment in MapLike")?;
        let key = crate::resources::identifier::read(name)?;
        let id = codec::registry_id("minecraft:enchantment", &key).map_err(|_| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment]: {key}"))?;
        Ok(SingleEnchantment { enchantment: codec::registry("minecraft:enchantment")[id as usize], level: IntProvider::parse(&json["level"])? })
    }

    pub fn enchant(&self, item_enchantments: &mut ItemEnchantments, random: &mut impl RandomSource) {
        let enchantment = Enchantment::by_id(self.enchantment).expect("a registered enchantment");
        item_enchantments.upgrade(self.enchantment, self.level.sample(random).clamp(enchantment.min_level(), enchantment.max_level()));
    }
}
