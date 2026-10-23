use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
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
    pub fn decode(tag: &Nbt) -> DataResult<IntValueCheck> {
        Ok(IntValueCheck {
            value: codec::decode_field(tag, "value", ContextIntProvider::decode_holder)?,
            range: codec::decode_field(tag, "test", IntRangePredicate::decode)?,
        })
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
