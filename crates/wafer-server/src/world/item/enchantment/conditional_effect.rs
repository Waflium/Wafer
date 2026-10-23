use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::component::types::{Pair, opt, record, req};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub fn codec(effect: Pair) -> Pair {
    record(vec![req("effect", effect), opt("requirements", LootItemCondition::holder_codec())])
}

pub struct ConditionalEffect<T> {
    pub effect: T,
    pub requirements: Option<Holder<LootItemCondition>>,
}

impl<T> ConditionalEffect<T> {
    pub fn decode<R: Into<DecodeResult<T>>>(tag: &Nbt, effect: impl FnOnce(&Nbt) -> R) -> DecodeResult<ConditionalEffect<T>> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = codec::Record::default();
        let effect = record.required(tag, "effect", effect);
        let requirements = record.optional(tag, "requirements", LootItemCondition::decode_holder_partial);
        record.build(|| Some(ConditionalEffect { effect: effect?, requirements }))
    }

    pub fn matches(&self, context: &mut LootContext) -> bool {
        self.requirements.as_ref().is_none_or(|requirements| requirements.value().test(context))
    }
}

impl<T> Validatable for ConditionalEffect<T> {
    fn validate(&self, context: &ValidationContext) {
        validatable::validate_optional_holder(context, "requirements", self.requirements.as_ref());
    }
}
