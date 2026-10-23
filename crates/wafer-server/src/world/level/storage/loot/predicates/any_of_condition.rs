use wafer_protocol::nbt::Nbt;

use super::loot_item_condition::LootItemCondition;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct AnyOfCondition {
    terms: Vec<Holder<LootItemCondition>>,
}

impl AnyOfCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<AnyOfCondition> {
        let mut record = codec::Record::default();
        let terms = record.required(tag, "terms", LootItemCondition::decode_holder_list_partial);
        record.build(|| Some(AnyOfCondition { terms: terms? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        self.terms.iter().any(|term| term.value().test(context))
    }
}

impl AnyOfCondition {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder_set(context, "terms", &self.terms);
    }
}
