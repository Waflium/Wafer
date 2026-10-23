use wafer_core::mth;
use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct SetItemDamageFunction {
    damage: Holder<ContextFloatProvider>,
    add: bool,
}

impl SetItemDamageFunction {
    pub fn decode(tag: &Nbt) -> DataResult<SetItemDamageFunction> {
        Ok(SetItemDamageFunction {
            damage: codec::decode_field(tag, "damage", ContextFloatProvider::decode_holder)?,
            add: codec::decode_or(tag, "add", false, codec::bool_value)?,
        })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if item_stack.is_damageable_item() {
            let max_damage = item_stack.max_damage();
            let base = if self.add { 1.0 - item_stack.damage_value() as f32 / max_damage as f32 } else { 0.0 };
            let pct = 1.0 - mth::clamp_f(self.damage.value().get_float(context) + base, 0.0, 1.0);
            item_stack.set_damage_value(mth::floor_f(pct * max_damage as f32));
        } else {
            warn!("Couldn't set damage of loot item {} {}", item_stack.count, item_stack.item.name());
        }
        item_stack
    }
}

impl SetItemDamageFunction {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "damage", &self.damage);
    }
}
