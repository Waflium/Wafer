use wafer_protocol::nbt::Nbt;

use super::loot_item_condition::LootItemCondition;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct InvertedLootItemCondition {
    term: Holder<LootItemCondition>,
}

impl InvertedLootItemCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<InvertedLootItemCondition> {
        let mut record = codec::Record::default();
        let term = record.required(tag, "term", LootItemCondition::decode_holder_partial);
        record.build(|| Some(InvertedLootItemCondition { term: term? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        !self.term.value().test(context)
    }
}

impl InvertedLootItemCondition {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "term", &self.term);
    }
}
