use wafer_protocol::nbt::Nbt;

use super::loot_item_function::LootItemFunction;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::container_component_manipulator::ContainerComponentManipulator;
use crate::world::level::storage::loot::container_component_manipulators;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct ModifyContainerContents {
    component: ContainerComponentManipulator,
    modifier: Holder<LootItemFunction>,
}

impl ModifyContainerContents {
    pub fn decode(tag: &Nbt) -> DecodeResult<ModifyContainerContents> {
        let mut record = codec::Record::default();
        let component = record.required(tag, "component", container_component_manipulators::decode);
        let modifier = record.required(tag, "modifier", LootItemFunction::decode_holder);
        record.build(|| Some(ModifyContainerContents { component: component?, modifier: modifier? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if item_stack.is_empty() {
            return item_stack;
        }
        self.component.modify_items(&mut item_stack, |c| self.modifier.value().apply(c, context));
        item_stack
    }
}

impl ModifyContainerContents {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "modifier", &self.modifier);
    }
}
