use wafer_protocol::nbt::Nbt;

use super::loot_item_function::LootItemFunction;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct SequenceFunction {
    functions: Vec<Holder<LootItemFunction>>,
}

impl SequenceFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SequenceFunction> {
        let mut record = codec::Record::default();
        let functions = record.required(tag, "functions", LootItemFunction::decode_holder_set);
        record.build(|| Some(SequenceFunction { functions: functions? }))
    }

    pub fn decode_inline(tag: &Nbt) -> DecodeResult<SequenceFunction> {
        LootItemFunction::decode_holder_set(tag).map(|functions| SequenceFunction { functions })
    }

    pub fn run(&self, mut item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        for function in &self.functions {
            item_stack = function.value().apply(item_stack, context);
        }
        item_stack
    }
}

impl SequenceFunction {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder_set(context, "functions", &self.functions);
    }
}
