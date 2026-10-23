use wafer_protocol::nbt::Nbt;

use super::loot_item_function::LootItemFunction;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct FilteredFunction {
    filter: ItemPredicate,
    on_pass: Option<Holder<LootItemFunction>>,
    on_fail: Option<Holder<LootItemFunction>>,
}

impl FilteredFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<FilteredFunction> {
        let mut record = codec::Record::default();
        let filter = record.required(tag, "item_filter", ItemPredicate::decode);
        let on_pass = record.optional(tag, "on_pass", LootItemFunction::decode_holder);
        let on_fail = record.optional(tag, "on_fail", LootItemFunction::decode_holder);
        record.build(|| Some(FilteredFunction { filter: filter?, on_pass, on_fail }))
    }

    pub fn run(&self, item_stack: ItemStack, context: &mut LootContext) -> ItemStack {
        let function = if self.filter.test(&item_stack) { &self.on_pass } else { &self.on_fail };
        match function {
            Some(function) => function.value().apply(item_stack, context),
            None => item_stack,
        }
    }
}

impl FilteredFunction {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_optional_holder(context, "on_pass", self.on_pass.as_ref());
        validatable::validate_optional_holder(context, "on_fail", self.on_fail.as_ref());
    }
}
