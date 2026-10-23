use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component::DataComponentPatch;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SetComponentsFunction {
    components: DataComponentPatch,
}

impl SetComponentsFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetComponentsFunction> {
        let mut record = codec::Record::default();
        let components = record.required(tag, "components", DataComponentPatch::from_nbt);
        record.build(|| Some(SetComponentsFunction { components: components? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        let _ = item_stack.apply_components_and_validate(&self.components);
        item_stack
    }
}
