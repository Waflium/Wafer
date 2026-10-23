use wafer_protocol::nbt::Nbt;

use super::loot_context::LootContext;
use super::providers::number::floats::{self, ContextFloatProvider};
use crate::core::holder::Holder;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::{Codec, Stream};
use crate::world::item::component::types::{self, Pair, opt};
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub fn codec() -> Pair {
    let line = types::record(vec![opt("min", floats::holder_codec()), opt("max", floats::holder_codec())]);
    (Codec::Either(Box::new(floats::holder_codec().0), Box::new(line.0)), Stream::Nbt)
}

pub enum FloatRangePredicate {
    Point(Holder<ContextFloatProvider>),
    Line { min: Option<Holder<ContextFloatProvider>>, max: Option<Holder<ContextFloatProvider>> },
}

impl FloatRangePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FloatRangePredicate> {
        match codec::either(tag, ContextFloatProvider::decode_holder, |t| {
            Ok((codec::decode_optional(t, "min", ContextFloatProvider::decode_holder)?, codec::decode_optional(t, "max", ContextFloatProvider::decode_holder)?))
        })? {
            Ok(point) => Ok(FloatRangePredicate::Point(point)),
            Err((min, max)) => Ok(FloatRangePredicate::Line { min, max }),
        }
    }

    pub fn test(&self, context: &mut LootContext, input: f32) -> bool {
        match self {
            FloatRangePredicate::Point(value) => value.value().get_float(context) == input,
            FloatRangePredicate::Line { min, max } => {
                if let Some(min) = min
                    && !(input >= min.value().get_float(context))
                {
                    return false;
                }
                max.as_ref().is_none_or(|max| input <= max.value().get_float(context))
            }
        }
    }

    pub fn test_provider(&self, context: &mut LootContext, input: &ContextFloatProvider) -> bool {
        match self {
            FloatRangePredicate::Line { min: None, max: None } => true,
            _ => {
                let input = input.get_float(context);
                self.test(context, input)
            }
        }
    }
}

impl Validatable for FloatRangePredicate {
    fn validate(&self, context: &ValidationContext) {
        match self {
            FloatRangePredicate::Line { min, max } => {
                validatable::validate_optional_holder(context, "min", min.as_ref());
                validatable::validate_optional_holder(context, "max", max.as_ref());
            }
            FloatRangePredicate::Point(value) => validatable::validate_holder_in(context, value),
        }
    }
}
