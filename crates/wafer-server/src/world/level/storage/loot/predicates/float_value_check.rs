use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::float_range_predicate::FloatRangePredicate;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::floats::ContextFloatProvider;
use crate::world::level::storage::loot::validatable;
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct FloatValueCheck {
    value: Holder<ContextFloatProvider>,
    range: FloatRangePredicate,
}

impl FloatValueCheck {
    pub fn decode(tag: &Nbt) -> DecodeResult<FloatValueCheck> {
        let mut record = codec::Record::default();
        let value = record.required(tag, "value", ContextFloatProvider::decode_holder);
        let range = record.required(tag, "test", FloatRangePredicate::decode);
        record.build(|| Some(FloatValueCheck { value: value?, range: range? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        self.range.test_provider(context, self.value.value())
    }
}

impl FloatValueCheck {
    pub fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "value", &self.value);
        validatable::validate(context, "range", &self.range);
    }
}
