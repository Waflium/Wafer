use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct EnchantmentActiveCheck {
    active: bool,
}

impl EnchantmentActiveCheck {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentActiveCheck> {
        Ok(EnchantmentActiveCheck { active: codec::decode_field(tag, "active", codec::bool_value)? })
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        context.params.enchantment_active.is_some_and(|value| value == self.active)
    }
}

impl EnchantmentActiveCheck {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        vec![LootContextParam::EnchantmentActive]
    }
}
