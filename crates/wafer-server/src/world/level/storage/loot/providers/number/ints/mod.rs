pub mod resolvable_int;

use std::sync::LazyLock;

use wafer_core::mth;
use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use super::floats::{self, ContextFloatProvider};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::{self, Pair, def, req};
use super::stored_number_access::StoredNumberAccess;
use super::{ArithmeticException, ArithmeticResult, Case, DIVISION_BY_ZERO, INTEGER_OVERFLOW, decode_holder_list_partial, decode_weighted_list, get_random};
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::providers::score::ScoreboardNameProvider;
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

fn direct_codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(direct_codec);
    &CODEC
}

pub fn holder_codec() -> Pair {
    (Codec::Either(Box::new(Codec::Identifier), Box::new(Codec::Lazy(direct_codec_lazy))), Stream::Nbt)
}

pub fn direct_codec() -> Pair {
    let input = |name: &'static str| req(name, holder_codec());
    let inputs = || req("inputs", types::compact_list(holder_codec()));
    let binary = || vec![input("left"), input("right")];
    let fallback = || def("fallback", holder_codec(), Nbt::Int(0));
    let cases = vec![
        ("minecraft:abs", vec![input("input")]),
        ("minecraft:avg", vec![inputs()]),
        ("minecraft:binomial", vec![input("n"), req("p", floats::holder_codec())]),
        (
            "minecraft:conditional",
            vec![req("condition", LootItemCondition::holder_codec()), input("on_true"), def("on_false", holder_codec(), Nbt::Int(0))],
        ),
        ("minecraft:constant", vec![req("value", (Codec::int(), Stream::Int))]),
        ("minecraft:sub", binary()),
        ("minecraft:environment_attribute", vec![req("attribute", types::by_name("minecraft:environment_attribute"))]),
        ("minecraft:from_float", vec![req("input", floats::holder_codec())]),
        ("minecraft:max", vec![inputs()]),
        ("minecraft:min", vec![inputs()]),
        ("minecraft:floor_mod", binary()),
        ("minecraft:floor_div", binary()),
        ("minecraft:mod", binary()),
        ("minecraft:div", binary()),
        ("minecraft:negate", vec![input("input")]),
        (
            "minecraft:number_dispatcher",
            vec![
                req("cases", types::list(types::record(vec![req("condition", LootItemCondition::holder_codec()), input("value")]))),
                def("default", holder_codec(), Nbt::Int(0)),
            ],
        ),
        ("minecraft:pow", vec![input("base"), input("exponent")]),
        ("minecraft:mul", vec![inputs()]),
        ("minecraft:score", vec![req("target", ScoreboardNameProvider::codec()), req("score", types::string()), fallback()]),
        ("minecraft:storage", vec![req("storage", types::identifier()), req("path", types::string()), fallback()]),
        ("minecraft:add", vec![inputs()]),
        ("minecraft:uniform", vec![input("min"), input("max")]),
        (
            "minecraft:weighted_list",
            vec![req("distribution", types::list(types::record(vec![req("data", holder_codec()), req("weight", types::var_int(types::non_negative_int()))])))],
        ),
    ];
    let typed = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:context_int_provider_type"), cases }));
    (Codec::Normalize(Box::new(Codec::Either(Box::new(Codec::int()), Box::new(typed))), constant_int), Stream::Nbt)
}

fn constant_int(provider: Nbt) -> Result<Nbt, String> {
    match provider.get("type").and_then(Nbt::as_str) {
        Some("minecraft:constant") => Ok(provider.get("value").cloned().expect("a constant value")),
        _ => Ok(provider),
    }
}

pub enum ContextIntProvider {
    Absolute(Holder<ContextIntProvider>),
    Average(Vec<Holder<ContextIntProvider>>),
    BinomialDistributionGenerator { n: Holder<ContextIntProvider>, p: Holder<ContextFloatProvider> },
    ConditionalValue { condition: Holder<LootItemCondition>, on_true: Holder<ContextIntProvider>, on_false: Holder<ContextIntProvider> },
    ConstantValue(i32),
    Difference { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    EnvironmentAttributeValue(&'static EnvironmentAttributeInfo),
    FromFloat(Holder<ContextFloatProvider>),
    Maximum(Vec<Holder<ContextIntProvider>>),
    Minimum(Vec<Holder<ContextIntProvider>>),
    FloorModulus { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    FloorQuotient { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    Modulus { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    Quotient { left: Holder<ContextIntProvider>, right: Holder<ContextIntProvider> },
    Negate(Holder<ContextIntProvider>),
    NumberDispatcher { cases: Vec<Case<ContextIntProvider>>, default_value: Holder<ContextIntProvider> },
    Power { base: Holder<ContextIntProvider>, exponent: Holder<ContextIntProvider> },
    Product(Vec<Holder<ContextIntProvider>>),
    ScoreboardValue { target: ScoreboardNameProvider, score: String, fallback: Holder<ContextIntProvider> },
    StorageValue { access: StoredNumberAccess, fallback: Holder<ContextIntProvider> },
    Sum(Vec<Holder<ContextIntProvider>>),
    UniformGenerator { min: Holder<ContextIntProvider>, max: Holder<ContextIntProvider> },
    WeightedListValue(Vec<(Holder<ContextIntProvider>, i32)>),
}

impl RegistryElement for ContextIntProvider {
    const REGISTRY: &'static str = "minecraft:context_int_provider";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static ContextIntProvider> {
        reloadable_server_registries::get().context_int_providers.get(key)
    }
}

pub fn exactly(value: i32) -> Holder<ContextIntProvider> {
    Holder::direct(ContextIntProvider::ConstantValue(value))
}

fn long_to_int_safe(value: i64) -> ArithmeticResult<i32> {
    i32::try_from(value).map_err(|_| ArithmeticException::new(format!("Value {value} can't be safely converted to int")))
}

pub fn float_to_int_safe(value: f32) -> ArithmeticResult<i32> {
    if !value.is_finite() {
        return Err(ArithmeticException::new(format!("Value {} can't be safely converted to int", crate::nbt::java_float_to_string(value))));
    }
    long_to_int_safe(value as i64)
}

fn overflow() -> ArithmeticException {
    ArithmeticException::new(INTEGER_OVERFLOW)
}

fn division_by_zero() -> ArithmeticException {
    ArithmeticException::new(DIVISION_BY_ZERO)
}

impl ContextIntProvider {
    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<ContextIntProvider>> {
        ContextIntProvider::decode_holder_partial(tag).into_result()
    }

    pub fn decode_holder_partial(tag: &Nbt) -> DecodeResult<Holder<ContextIntProvider>> {
        Holder::decode_partial(tag, ContextIntProvider::decode_partial)
    }

    fn decode_holders(tag: &Nbt) -> DecodeResult<Vec<Holder<ContextIntProvider>>> {
        decode_holder_list_partial(tag, ContextIntProvider::decode_holder_partial)
    }

    pub fn decode(tag: &Nbt) -> DataResult<ContextIntProvider> {
        ContextIntProvider::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<ContextIntProvider> {
        codec::either_result(DecodeResult::from(codec::int(tag)), || ContextIntProvider::decode_typed(tag)).map(|value| match value {
            Ok(value) => ContextIntProvider::ConstantValue(value),
            Err(provider) => provider,
        })
    }

    fn decode_typed(tag: &Nbt) -> DecodeResult<ContextIntProvider> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let ty = match codec::decode_field(tag, "type", codec::identifier) {
            Ok(ty) => ty,
            Err(error) => return DecodeResult::error(error),
        };
        let mut record = Record::default();
        let holder = |record: &mut Record, key: &str| record.required(tag, key, ContextIntProvider::decode_holder_partial);
        let fallback = |record: &mut Record| record.with_default(tag, "fallback", exactly(0), ContextIntProvider::decode_holder_partial);
        match ty.as_str() {
            "minecraft:abs" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextIntProvider::Absolute(input?)))
            }
            "minecraft:avg" => {
                let inputs = record.required(tag, "inputs", ContextIntProvider::decode_holders);
                record.build(|| Some(ContextIntProvider::Average(inputs?)))
            }
            "minecraft:binomial" => {
                let n = holder(&mut record, "n");
                let p = record.required(tag, "p", ContextFloatProvider::decode_holder_partial);
                record.build(|| Some(ContextIntProvider::BinomialDistributionGenerator { n: n?, p: p? }))
            }
            "minecraft:conditional" => {
                let condition = record.required(tag, "condition", LootItemCondition::decode_holder_partial);
                let on_true = holder(&mut record, "on_true");
                let on_false = record.with_default(tag, "on_false", exactly(0), ContextIntProvider::decode_holder_partial);
                record.build(|| Some(ContextIntProvider::ConditionalValue { condition: condition?, on_true: on_true?, on_false }))
            }
            "minecraft:constant" => {
                let value = record.required(tag, "value", codec::int);
                record.build(|| Some(ContextIntProvider::ConstantValue(value?)))
            }
            "minecraft:sub" => {
                let left = holder(&mut record, "left");
                let right = holder(&mut record, "right");
                record.build(|| Some(ContextIntProvider::Difference { left: left?, right: right? }))
            }
            "minecraft:environment_attribute" => {
                let attribute = record.required(tag, "attribute", |t| -> DataResult<&'static EnvironmentAttributeInfo> {
                    let name = codec::identifier(t)?;
                    let attribute = EnvironmentAttributeInfo::by_name(&name).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:environment_attribute]: {name}"))?;
                    if attribute.ty.to_int(attribute.default_value()).is_none() {
                        return Err(format!("{name} cannot be converted to an integer"));
                    }
                    Ok(attribute)
                });
                record.build(|| Some(ContextIntProvider::EnvironmentAttributeValue(attribute?)))
            }
            "minecraft:from_float" => {
                let input = record.required(tag, "input", ContextFloatProvider::decode_holder_partial);
                record.build(|| Some(ContextIntProvider::FromFloat(input?)))
            }
            "minecraft:max" => {
                let inputs = record.required(tag, "inputs", ContextIntProvider::decode_holders);
                record.build(|| Some(ContextIntProvider::Maximum(inputs?)))
            }
            "minecraft:min" => {
                let inputs = record.required(tag, "inputs", ContextIntProvider::decode_holders);
                record.build(|| Some(ContextIntProvider::Minimum(inputs?)))
            }
            "minecraft:floor_mod" => {
                let left = holder(&mut record, "left");
                let right = holder(&mut record, "right");
                record.build(|| Some(ContextIntProvider::FloorModulus { left: left?, right: right? }))
            }
            "minecraft:floor_div" => {
                let left = holder(&mut record, "left");
                let right = holder(&mut record, "right");
                record.build(|| Some(ContextIntProvider::FloorQuotient { left: left?, right: right? }))
            }
            "minecraft:mod" => {
                let left = holder(&mut record, "left");
                let right = holder(&mut record, "right");
                record.build(|| Some(ContextIntProvider::Modulus { left: left?, right: right? }))
            }
            "minecraft:div" => {
                let left = holder(&mut record, "left");
                let right = holder(&mut record, "right");
                record.build(|| Some(ContextIntProvider::Quotient { left: left?, right: right? }))
            }
            "minecraft:negate" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextIntProvider::Negate(input?)))
            }
            "minecraft:number_dispatcher" => {
                let cases = record.required(tag, "cases", |t| codec::list_partial(t, |c| Case::decode_partial(c, ContextIntProvider::decode_holder_partial)));
                let default_value = record.with_default(tag, "default", exactly(0), ContextIntProvider::decode_holder_partial);
                record.build(|| Some(ContextIntProvider::NumberDispatcher { cases: cases?, default_value }))
            }
            "minecraft:pow" => {
                let base = holder(&mut record, "base");
                let exponent = holder(&mut record, "exponent");
                record.build(|| Some(ContextIntProvider::Power { base: base?, exponent: exponent? }))
            }
            "minecraft:mul" => {
                let inputs = record.required(tag, "inputs", ContextIntProvider::decode_holders);
                record.build(|| Some(ContextIntProvider::Product(inputs?)))
            }
            "minecraft:score" => {
                let target = record.required(tag, "target", ScoreboardNameProvider::decode);
                let score = record.required(tag, "score", |t| codec::string(t).map(str::to_string));
                let fallback = fallback(&mut record);
                record.build(|| Some(ContextIntProvider::ScoreboardValue { target: target?, score: score?, fallback }))
            }
            "minecraft:storage" => {
                let access = record.absorb(StoredNumberAccess::decode_partial(tag));
                let fallback = fallback(&mut record);
                record.build(|| Some(ContextIntProvider::StorageValue { access: access?, fallback }))
            }
            "minecraft:add" => {
                let inputs = record.required(tag, "inputs", ContextIntProvider::decode_holders);
                record.build(|| Some(ContextIntProvider::Sum(inputs?)))
            }
            "minecraft:uniform" => {
                let min = holder(&mut record, "min");
                let max = holder(&mut record, "max");
                record.build(|| Some(ContextIntProvider::UniformGenerator { min: min?, max: max? }))
            }
            "minecraft:weighted_list" => {
                let distribution = record.required(tag, "distribution", |t| decode_weighted_list(t, ContextIntProvider::decode_holder_partial));
                record.build(|| Some(ContextIntProvider::WeightedListValue(distribution?)))
            }
            other => DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:context_int_provider_type]: {other}")),
        }
    }

    pub fn get_int(&self, context: &mut LootContext) -> i32 {
        self.get_int_unsafe(context).unwrap_or(0)
    }

    pub fn get_int_unsafe(&self, context: &mut LootContext) -> ArithmeticResult<i32> {
        match self {
            ContextIntProvider::Absolute(input) => {
                input.value().get_int_unsafe(context)?.checked_abs().ok_or_else(|| ArithmeticException::new("Overflow to represent absolute value of Integer.MIN_VALUE"))
            }
            ContextIntProvider::Average(inputs) => {
                let (mut sum, mut count) = (0i64, 0i64);
                for input in inputs {
                    sum = sum.wrapping_add(input.value().get_int_unsafe(context)? as i64);
                    count += 1;
                }
                if count == 0 {
                    return Err(division_by_zero());
                }
                long_to_int_safe(sum / count)
            }
            ContextIntProvider::BinomialDistributionGenerator { n, p } => {
                let n = n.value().get_int_unsafe(context)?;
                let p = p.value().get_float_or_throw(context)?;
                let mut result = 0;
                for _ in 0..n {
                    if context.random().next_float() < p {
                        result += 1;
                    }
                }
                Ok(result)
            }
            ContextIntProvider::ConditionalValue { condition, on_true, on_false } => {
                let value = if condition.value().test(context) { on_true } else { on_false };
                value.value().get_int_unsafe(context)
            }
            ContextIntProvider::ConstantValue(value) => Ok(*value),
            ContextIntProvider::Difference { left, right } => {
                left.value().get_int_unsafe(context)?.checked_sub(right.value().get_int_unsafe(context)?).ok_or_else(overflow)
            }
            ContextIntProvider::EnvironmentAttributeValue(attribute) => {
                let value = context.environment_attribute_value(attribute);
                attribute.ty.to_int(&value).ok_or_else(|| ArithmeticException::new(format!("{value:?} cannot be represented as an integer")))
            }
            ContextIntProvider::FromFloat(input) => float_to_int_safe(input.value().get_float_unsafe(context)?),
            ContextIntProvider::Maximum(inputs) => {
                let mut value = i32::MIN;
                for input in inputs {
                    value = value.max(input.value().get_int_unsafe(context)?);
                }
                Ok(value)
            }
            ContextIntProvider::Minimum(inputs) => {
                let mut value = i32::MAX;
                for input in inputs {
                    value = value.min(input.value().get_int_unsafe(context)?);
                }
                Ok(value)
            }
            ContextIntProvider::FloorModulus { left, right } => {
                let (left, right) = (left.value().get_int_unsafe(context)?, right.value().get_int_unsafe(context)?);
                if right == 0 {
                    return Err(division_by_zero());
                }
                let modulus = left.wrapping_rem(right);
                Ok(if modulus != 0 && (modulus ^ right) < 0 { modulus + right } else { modulus })
            }
            ContextIntProvider::FloorQuotient { left, right } => {
                let (left, right) = (left.value().get_int_unsafe(context)?, right.value().get_int_unsafe(context)?);
                if right == 0 {
                    return Err(division_by_zero());
                }
                if left == i32::MIN && right == -1 {
                    return Err(overflow());
                }
                let quotient = left / right;
                Ok(if (left ^ right) < 0 && quotient * right != left { quotient - 1 } else { quotient })
            }
            ContextIntProvider::Modulus { left, right } => {
                let (left, right) = (left.value().get_int_unsafe(context)?, right.value().get_int_unsafe(context)?);
                if right == 0 {
                    return Err(division_by_zero());
                }
                Ok(left.wrapping_rem(right))
            }
            ContextIntProvider::Quotient { left, right } => {
                let (left, right) = (left.value().get_int_unsafe(context)?, right.value().get_int_unsafe(context)?);
                if right == 0 {
                    return Err(division_by_zero());
                }
                Ok(left.wrapping_div(right))
            }
            ContextIntProvider::Negate(input) => input.value().get_int_unsafe(context)?.checked_neg().ok_or_else(overflow),
            ContextIntProvider::NumberDispatcher { cases, default_value } => {
                for case in cases {
                    if case.test(context) {
                        return case.value.value().get_int_unsafe(context);
                    }
                }
                default_value.value().get_int_unsafe(context)
            }
            ContextIntProvider::Power { base, exponent } => {
                let (base, exponent) = (base.value().get_int_unsafe(context)?, exponent.value().get_int_unsafe(context)?);
                if base == 0 && exponent == 0 {
                    return Err(ArithmeticException::new("Result of 0 to the power of 0 is undefined"));
                }
                if exponent < 0 {
                    return Err(ArithmeticException::new("negative exponent"));
                }
                base.checked_pow(exponent as u32).ok_or_else(overflow)
            }
            ContextIntProvider::Product(inputs) => {
                let mut value = 1i64;
                for input in inputs {
                    value = value.wrapping_mul(input.value().get_int_unsafe(context)? as i64);
                }
                long_to_int_safe(value)
            }
            ContextIntProvider::ScoreboardValue { target, score, fallback } => match target.score_value(context, score) {
                Some(value) => Ok(value),
                None => fallback.value().get_int_unsafe(context),
            },
            ContextIntProvider::StorageValue { access, fallback } => match access.get_numeric_tag(context) {
                Some(value) => Ok(crate::world::item::component::codec::as_i32(&value)),
                None => fallback.value().get_int_unsafe(context),
            },
            ContextIntProvider::Sum(inputs) => {
                let mut value = 0i64;
                for input in inputs {
                    value = value.wrapping_add(input.value().get_int_unsafe(context)? as i64);
                }
                long_to_int_safe(value)
            }
            ContextIntProvider::UniformGenerator { min, max } => {
                let (min, max) = (min.value().get_int_unsafe(context)?, max.value().get_int_unsafe(context)?);
                Ok(mth::next_int(&mut context.random(),min, max))
            }
            ContextIntProvider::WeightedListValue(distribution) => get_random(distribution, context).value().get_int_unsafe(context),
        }
    }
}

impl Validatable for ContextIntProvider {
    fn validate(&self, context: &ValidationContext) {
        match self {
            ContextIntProvider::Absolute(input) | ContextIntProvider::Negate(input) => validatable::validate_holder(context, "input", input),
            ContextIntProvider::FromFloat(input) => validatable::validate_holder(context, "input", input),
            ContextIntProvider::Average(inputs)
            | ContextIntProvider::Maximum(inputs)
            | ContextIntProvider::Minimum(inputs)
            | ContextIntProvider::Product(inputs)
            | ContextIntProvider::Sum(inputs) => validatable::validate_holder_set_with_min_size(context, "inputs", inputs, 1),
            ContextIntProvider::BinomialDistributionGenerator { n, p } => {
                validatable::validate_holder(context, "n", n);
                validatable::validate_holder(context, "p", p);
            }
            ContextIntProvider::ConditionalValue { condition, on_true, on_false } => {
                validatable::validate_holder(context, "condition", condition);
                validatable::validate_holder(context, "on_true", on_true);
                validatable::validate_holder(context, "on_false", on_false);
            }
            ContextIntProvider::ConstantValue(_) => {}
            ContextIntProvider::Difference { left, right }
            | ContextIntProvider::FloorModulus { left, right }
            | ContextIntProvider::FloorQuotient { left, right }
            | ContextIntProvider::Modulus { left, right }
            | ContextIntProvider::Quotient { left, right } => {
                validatable::validate_holder(context, "left", left);
                validatable::validate_holder(context, "right", right);
            }
            ContextIntProvider::EnvironmentAttributeValue(attribute) => {
                context.validate_context_usage(if attribute.is_positional { &[LootContextParam::Origin] } else { &[] })
            }
            ContextIntProvider::NumberDispatcher { cases, default_value } => {
                validatable::validate_list(context, "cases", cases);
                validatable::validate_holder(context, "default", default_value);
            }
            ContextIntProvider::Power { base, exponent } => {
                validatable::validate_holder(context, "base", base);
                validatable::validate_holder(context, "exponent", exponent);
            }
            ContextIntProvider::ScoreboardValue { target, fallback, .. } => {
                validatable::validate(context, "target", target);
                validatable::validate_holder(context, "fallback", fallback);
            }
            ContextIntProvider::StorageValue { fallback, .. } => validatable::validate_holder(context, "fallback", fallback),
            ContextIntProvider::UniformGenerator { min, max } => {
                validatable::validate_holder(context, "min", min);
                validatable::validate_holder(context, "max", max);
            }
            ContextIntProvider::WeightedListValue(distribution) => {
                let values: Vec<&Holder<ContextIntProvider>> = distribution.iter().map(|(value, _)| value).collect();
                validatable::validate_holder_refs(context, "distribution", &values);
            }
        }
    }
}
