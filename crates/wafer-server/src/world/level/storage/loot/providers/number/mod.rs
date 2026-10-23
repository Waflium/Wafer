pub mod floats;
pub mod ints;
pub mod stored_number_access;

use wafer_protocol::nbt::Nbt;

use crate::core::holder::{Holder, RegistryElement};
use crate::util::codec::{self, DecodeResult, Record};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

#[derive(Clone, Debug)]
pub struct ArithmeticException(pub String);

impl ArithmeticException {
    pub fn new(message: impl Into<String>) -> ArithmeticException {
        ArithmeticException(message.into())
    }

    pub fn get_message(&self) -> &str {
        &self.0
    }
}

pub const INTEGER_OVERFLOW: &str = "integer overflow";
pub const DIVISION_BY_ZERO: &str = "/ by zero";

pub type ArithmeticResult<T> = Result<T, ArithmeticException>;

pub struct Case<V: RegistryElement> {
    condition: Holder<LootItemCondition>,
    pub value: Holder<V>,
}

impl<V: RegistryElement> Case<V> {
    pub fn decode_partial(tag: &Nbt, value: fn(&Nbt) -> DecodeResult<Holder<V>>) -> DecodeResult<Case<V>> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let condition = record.required(tag, "condition", LootItemCondition::decode_holder_partial);
        let value = record.required(tag, "value", value);
        record.build(|| Some(Case { condition: condition?, value: value? }))
    }

    pub fn test(&self, context: &mut LootContext) -> bool {
        self.condition.value().test(context)
    }
}

pub fn decode_holder_list_partial<V: RegistryElement>(tag: &Nbt, holder: fn(&Nbt) -> DecodeResult<Holder<V>>) -> DecodeResult<Vec<Holder<V>>> {
    codec::holder_set_partial(tag, V::REGISTRY, holder)
}

pub fn decode_weighted_list<V: RegistryElement>(tag: &Nbt, holder: fn(&Nbt) -> DecodeResult<Holder<V>>) -> DecodeResult<Vec<(Holder<V>, i32)>> {
    codec::list_partial(tag, |entry| {
        if let Err(error) = codec::map(entry) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let data = record.required(entry, "data", holder);
        let weight = record.required(entry, "weight", |t| {
            let weight = codec::int(t)?;
            if weight < 0 { Err(format!("Value must be non-negative: {weight}")) } else { Ok(weight) }
        });
        record.build(|| Some((data?, weight?)))
    })
    .validate(|entries| if entries.is_empty() { Err("Weighted list must contain at least one entry with non-zero weight".to_string()) } else { Ok(()) })
}

pub fn get_random<'v, V>(distribution: &'v [(V, i32)], context: &mut LootContext) -> &'v V {
    use wafer_core::random::RandomSource;
    let total_weight: i32 = distribution.iter().map(|(_, weight)| *weight).sum();
    let mut selection = context.random().next_int_bounded(total_weight);
    for (value, weight) in distribution {
        selection -= weight;
        if selection < 0 {
            return value;
        }
    }
    unreachable!("selection within the total weight")
}

impl<V: RegistryElement + Validatable> Validatable for Case<V> {
    fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "condition", &self.condition);
        validatable::validate_holder(context, "value", &self.value);
    }
}
