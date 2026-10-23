use wafer_protocol::nbt::Nbt;

use super::loot_item_condition::LootItemCondition;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct AllOfCondition {
    terms: Vec<Holder<LootItemCondition>>,
}

impl AllOfCondition {
    pub fn decode(tag: &Nbt) -> DecodeResult<AllOfCondition> {
        let mut record = codec::Record::default();
        let terms = record.required(tag, "terms", LootItemCondition::decode_holder_list_partial);
        record.build(|| Some(AllOfCondition { terms: terms? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        self.terms.iter().all(|term| term.value().test(context))
    }
}

impl AllOfCondition {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder_set(context, "terms", &self.terms);
    }
}
