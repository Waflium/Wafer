use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::container_component_manipulator::ContainerComponentManipulator;
use crate::world::level::storage::loot::container_component_manipulators;
use crate::world::level::storage::loot::entries::loot_pool_entries;
use crate::world::level::storage::loot::entries::loot_pool_entry_container::LootPoolEntryContainer;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::loot_table;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct SetContainerContents {
    component: ContainerComponentManipulator,
    entries: Vec<LootPoolEntryContainer>,
}

impl SetContainerContents {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetContainerContents> {
        let mut record = codec::Record::default();
        let component = record.required(tag, "component", container_component_manipulators::decode);
        let entries = record.required(tag, "entries", |t| codec::list_partial(t, |entry| DecodeResult::from(loot_pool_entries::decode(entry))));
        record.build(|| Some(SetContainerContents { component: component?, entries: entries? }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        if item_stack.is_empty() {
            return item_stack;
        }
        let mut contents = Vec::new();
        {
            let mut add = |_: &mut LootContext, item_stack: ItemStack| contents.push(item_stack);
            let mut splitter = loot_table::create_stack_splitter(&mut add);
            for e in &self.entries {
                e.expand(context, &[], &mut |context, entry| entry.create_item_stack(&mut splitter, context));
            }
        }
        self.component.set_contents(&mut item_stack, contents);
        item_stack
    }
}

impl SetContainerContents {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_list(context, "entries", &self.entries);
    }
}
