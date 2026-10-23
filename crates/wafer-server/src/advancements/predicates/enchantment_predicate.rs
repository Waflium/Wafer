use wafer_protocol::nbt::Nbt;

use super::min_max_bounds::ints_codec;
use crate::world::item::component::types::{self, Pair, def, opt, record};

pub fn codec() -> Pair {
    record(vec![opt("enchantments", types::holder_set("minecraft:enchantment")), def("levels", ints_codec(), types::empty_map())])
}

use super::min_max_bounds::Ints;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::registry;
use crate::world::item::component::values::ItemEnchantments;

#[derive(Debug)]
pub struct EnchantmentPredicate {
    enchantments: Option<HolderSet>,
    level: Ints,
}

impl EnchantmentPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let enchantments = record.field(codec::decode_optional(tag, "enchantments", |t| HolderSet::decode("minecraft:enchantment", t)));
        let level = record.field(codec::decode_or(tag, "levels", Ints::ANY, Ints::decode));
        record.finish()?;
        Ok(EnchantmentPredicate { enchantments: enchantments.unwrap(), level: level.unwrap() })
    }

    pub fn contained_in(&self, item_enchantments: &ItemEnchantments) -> bool {
        if let Some(enchantments) = &self.enchantments {
            let names = registry("minecraft:enchantment");
            enchantments.contents().iter().any(|&id| self.matches_enchantment(item_enchantments, names[id as usize]))
        } else if !self.level.is_any() {
            item_enchantments.enchantments.iter().any(|(_, level)| self.level.matches(*level))
        } else {
            !item_enchantments.is_empty()
        }
    }

    fn matches_enchantment(&self, item_enchantments: &ItemEnchantments, enchantment: &str) -> bool {
        let level = item_enchantments.level(enchantment);
        level != 0 && (self.level.is_any() || self.level.matches(level))
    }
}
