use wafer_protocol::nbt::Nbt;

use super::loot_context::LootContext;
use super::providers::number::ints::ContextIntProvider;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct IntLimit {
    min: Option<Holder<ContextIntProvider>>,
    max: Option<Holder<ContextIntProvider>>,
}

impl IntLimit {
    pub fn decode(tag: &Nbt) -> DecodeResult<IntLimit> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let min = record.optional(tag, "min", ContextIntProvider::decode_holder_partial);
            let max = record.optional(tag, "max", ContextIntProvider::decode_holder_partial);
            record.build(|| Some(IntLimit { min, max }))
        })
    }

    pub fn clamp(&self, context: &mut LootContext, input: i32) -> i32 {
        match (&self.min, &self.max) {
            (None, None) => input,
            (None, Some(max)) => max.value().get_int(context).min(input),
            (Some(min), None) => min.value().get_int(context).max(input),
            (Some(min), Some(max)) => {
                let min = min.value().get_int(context);
                let max = max.value().get_int(context);
                if input < min { min } else { input.min(max) }
            }
        }
    }
}

impl Validatable for IntLimit {
    fn validate(&self, context: &ValidationContext) {
        validatable::validate_optional_holder(context, "min", self.min.as_ref());
        validatable::validate_optional_holder(context, "max", self.max.as_ref());
    }
}
