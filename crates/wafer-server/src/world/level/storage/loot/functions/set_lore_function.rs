use wafer_protocol::nbt::Nbt;

use super::list_operation::ListOperation;
use super::set_name_function::{SetNameFunction, decode_component};
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::storage::loot::loot_context::{EntityTarget, LootContext};
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub struct SetLoreFunction {
    lore: Vec<Nbt>,
    mode: ListOperation,
    resolution_context: Option<EntityTarget>,
}

impl SetLoreFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SetLoreFunction> {
        let mut record = codec::Record::default();
        let lore = record.required(tag, "lore", |t| codec::sized_list_partial(t, 256, decode_component));
        let mode = record.nested(|| ListOperation::decode(tag, 256));
        let resolution_context = record.optional(tag, "entity", EntityTarget::decode);
        record.build(|| Some(SetLoreFunction { lore: lore?, mode: mode?, resolution_context }))
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let old_lore = item_stack.get(dc::LORE).map_or(Vec::new(), |l| l.as_list().to_vec());
        let resolved_lines: Vec<Nbt> = self.lore.iter().map(SetNameFunction::create_resolver(context, self.resolution_context)).collect();
        let lines = self.mode.apply(&old_lore, &resolved_lines, 256);
        item_stack.set(dc::LORE, Some(Nbt::List(lines)));
        item_stack
    }
}

impl SetLoreFunction {
    pub fn get_referenced_context_params(&self) -> Vec<LootContextParam> {
        self.resolution_context.map(|target| vec![target.context_param()]).unwrap_or_default()
    }
}
