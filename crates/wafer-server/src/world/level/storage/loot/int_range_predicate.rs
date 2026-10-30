use wafer_protocol::nbt::Nbt;

use super::loot_context::LootContext;
use super::providers::number::ints::{self, ContextIntProvider};
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::component::codec::Wrap;
use crate::world::item::component::codec::{Codec, Stream};
use crate::world::item::component::types::{self, Pair, opt};
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub fn codec() -> Pair {
    let line = types::record(vec![opt("min", ints::holder_codec()), opt("max", ints::holder_codec())]);
    (Codec::Either(Box::new(ints::holder_codec().0), Box::new(line.0)), Stream::Nbt)
}

pub enum IntRangePredicate {
    Point(Holder<ContextIntProvider>),
    Line { min: Option<Holder<ContextIntProvider>>, max: Option<Holder<ContextIntProvider>> },
}

impl IntRangePredicate {
    pub fn decode(tag: &Nbt) -> DecodeResult<IntRangePredicate> {
        codec::wrapped(Wrap::Xmap, || {
            codec::either_result(
                codec::either_first(|| codec::wrapped(Wrap::Xmap, || ContextIntProvider::decode_holder_partial(tag))),
                || {
                    codec::either_second(|| {
                        codec::as_codec(|| {
                            if let Err(error) = codec::map(tag) {
                                return DecodeResult::error(error);
                            }
                            let mut record = codec::Record::default();
                            let min = record.optional(tag, "min", ContextIntProvider::decode_holder_partial);
                            let max = record.optional(tag, "max", ContextIntProvider::decode_holder_partial);
                            record.build(|| Some((min, max)))
                        })
                    })
                },
            )
        })
        .map(|either| match either {
            Ok(point) => IntRangePredicate::Point(point),
            Err((min, max)) => IntRangePredicate::Line { min, max },
        })
    }

    pub fn test(&self, context: &mut LootContext, input: i32) -> bool {
        match self {
            IntRangePredicate::Point(value) => value.value().get_int(context) == input,
            IntRangePredicate::Line { min, max } => {
                if let Some(min) = min
                    && input < min.value().get_int(context)
                {
                    return false;
                }
                max.as_ref().is_none_or(|max| input <= max.value().get_int(context))
            }
        }
    }

    pub fn test_provider(&self, context: &mut LootContext, input: &ContextIntProvider) -> bool {
        match self {
            IntRangePredicate::Line { min: None, max: None } => true,
            _ => {
                let input = input.get_int(context);
                self.test(context, input)
            }
        }
    }
}

impl Validatable for IntRangePredicate {
    fn validate(&self, context: &ValidationContext) {
        match self {
            IntRangePredicate::Line { min, max } => {
                validatable::validate_optional_holder(context, "min", min.as_ref());
                validatable::validate_optional_holder(context, "max", max.as_ref());
            }
            IntRangePredicate::Point(value) => validatable::validate_holder_in(context, value),
        }
    }
}
