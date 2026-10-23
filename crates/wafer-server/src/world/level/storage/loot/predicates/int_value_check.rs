use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::int_range_predicate::IntRangePredicate;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct IntValueCheck {
    value: Holder<ContextIntProvider>,
    range: IntRangePredicate,
}

impl IntValueCheck {
    pub fn decode(tag: &Nbt) -> DecodeResult<IntValueCheck> {
        let mut record = codec::Record::default();
        let value = record.required(tag, "value", ContextIntProvider::decode_holder);
        let range = record.required(tag, "test", IntRangePredicate::decode);
        record.build(|| Some(IntValueCheck { value: value?, range: range? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        self.range.test_provider(context, self.value.value())
    }
}

impl IntValueCheck {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "value", &self.value);
        validatable::validate(context, "range", &self.range);
    }
}
