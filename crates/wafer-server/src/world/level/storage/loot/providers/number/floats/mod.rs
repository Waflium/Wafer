pub mod resolvable_float;

use wafer_core::mth;
use wafer_protocol::nbt::Nbt;

use super::ints::ContextIntProvider;
use super::stored_number_access::StoredNumberAccess;
use super::{ArithmeticException, ArithmeticResult, Case, decode_holder_list_partial, decode_weighted_list, get_random};
use crate::core::holder::{Holder, RegistryElement};
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::util::{java_max_f, java_min_f};
use crate::world::attribute::environment_attribute::EnvironmentAttributeInfo;
use crate::world::item::enchantment::level_based_value::LevelBasedValue;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;
use crate::world::level::storage::loot::parameters::loot_context_params::LootContextParam;

pub enum ContextFloatProvider {
    Absolute(Holder<ContextFloatProvider>),
    Average(Vec<Holder<ContextFloatProvider>>),
    Ceiling(Holder<ContextFloatProvider>),
    ConditionalValue { condition: Holder<LootItemCondition>, on_true: Holder<ContextFloatProvider>, on_false: Holder<ContextFloatProvider> },
    ConstantValue(f32),
    Cosine(Holder<ContextFloatProvider>),
    Difference { left: Holder<ContextFloatProvider>, right: Holder<ContextFloatProvider> },
    EnchantmentLevelProvider(LevelBasedValue),
    EnvironmentAttributeValue(&'static EnvironmentAttributeInfo),
    Floor(Holder<ContextFloatProvider>),
    FromInt(Holder<ContextIntProvider>),
    Length(Vec<Holder<ContextFloatProvider>>),
    Maximum(Vec<Holder<ContextFloatProvider>>),
    Minimum(Vec<Holder<ContextFloatProvider>>),
    Modulus { left: Holder<ContextFloatProvider>, right: Holder<ContextFloatProvider> },
    Negate(Holder<ContextFloatProvider>),
    NumberDispatcher { cases: Vec<Case<ContextFloatProvider>>, default_value: Holder<ContextFloatProvider> },
    Power { base: Holder<ContextFloatProvider>, exponent: Holder<ContextFloatProvider> },
    Product(Vec<Holder<ContextFloatProvider>>),
    Quotient { left: Holder<ContextFloatProvider>, right: Holder<ContextFloatProvider> },
    Round(Holder<ContextFloatProvider>),
    Sine(Holder<ContextFloatProvider>),
    SquareRoot(Holder<ContextFloatProvider>),
    StorageValue { access: StoredNumberAccess, fallback: Holder<ContextFloatProvider> },
    Sum(Vec<Holder<ContextFloatProvider>>),
    Truncate(Holder<ContextFloatProvider>),
    UniformGenerator { min: Holder<ContextFloatProvider>, max: Holder<ContextFloatProvider> },
    WeightedListValue(Vec<(Holder<ContextFloatProvider>, i32)>),
}

impl RegistryElement for ContextFloatProvider {
    const REGISTRY: &'static str = "minecraft:context_float_provider";

    fn contains_key(key: &str) -> bool {
        reloadable_server_registries::contains_key(Self::REGISTRY, key)
    }

    fn lookup(key: &str) -> Option<&'static ContextFloatProvider> {
        reloadable_server_registries::get().context_float_providers.get(key)
    }
}

pub fn exactly(value: f32) -> Holder<ContextFloatProvider> {
    Holder::direct(ContextFloatProvider::ConstantValue(value))
}

impl ContextFloatProvider {
    pub fn decode_holder(tag: &Nbt) -> DataResult<Holder<ContextFloatProvider>> {
        ContextFloatProvider::decode_holder_partial(tag).into_result()
    }

    pub fn decode_holder_partial(tag: &Nbt) -> DecodeResult<Holder<ContextFloatProvider>> {
        Holder::decode_partial(tag, ContextFloatProvider::decode_partial)
    }

    fn decode_holders(tag: &Nbt) -> DecodeResult<Vec<Holder<ContextFloatProvider>>> {
        decode_holder_list_partial(tag, ContextFloatProvider::decode_holder_partial)
    }

    pub fn decode(tag: &Nbt) -> DataResult<ContextFloatProvider> {
        ContextFloatProvider::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<ContextFloatProvider> {
        codec::either_result(DecodeResult::from(codec::float(tag)), || ContextFloatProvider::decode_typed(tag)).map(|value| match value {
            Ok(value) => ContextFloatProvider::ConstantValue(value),
            Err(provider) => provider,
        })
    }

    fn decode_typed(tag: &Nbt) -> DecodeResult<ContextFloatProvider> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let ty = match codec::decode_field(tag, "type", codec::identifier) {
            Ok(ty) => ty,
            Err(error) => return DecodeResult::error(error),
        };
        let mut record = Record::default();
        let holder = |record: &mut Record, key: &str| record.required(tag, key, ContextFloatProvider::decode_holder_partial);
        let fallback = |record: &mut Record| record.with_default(tag, "fallback", exactly(0.0), ContextFloatProvider::decode_holder_partial);
        match ty.as_str() {
            "minecraft:abs" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::Absolute(input?)))
            }
            "minecraft:avg" => {
                let inputs = record.required(tag, "inputs", ContextFloatProvider::decode_holders);
                record.build(|| Some(ContextFloatProvider::Average(inputs?)))
            }
            "minecraft:ceil" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::Ceiling(input?)))
            }
            "minecraft:conditional" => {
                let condition = record.required(tag, "condition", LootItemCondition::decode_holder_partial);
                let on_true = holder(&mut record, "on_true");
                let on_false = record.with_default(tag, "on_false", exactly(0.0), ContextFloatProvider::decode_holder_partial);
                record.build(|| Some(ContextFloatProvider::ConditionalValue { condition: condition?, on_true: on_true?, on_false }))
            }
            "minecraft:constant" => {
                let value = record.required(tag, "value", codec::float);
                record.build(|| Some(ContextFloatProvider::ConstantValue(value?)))
            }
            "minecraft:cos" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::Cosine(input?)))
            }
            "minecraft:sub" => {
                let left = holder(&mut record, "left");
                let right = holder(&mut record, "right");
                record.build(|| Some(ContextFloatProvider::Difference { left: left?, right: right? }))
            }
            "minecraft:enchantment_level" => {
                let amount = record.required(tag, "amount", LevelBasedValue::decode_partial);
                record.build(|| Some(ContextFloatProvider::EnchantmentLevelProvider(amount?)))
            }
            "minecraft:environment_attribute" => {
                let attribute = record.required(tag, "attribute", |t| -> DataResult<&'static EnvironmentAttributeInfo> {
                    let name = codec::identifier(t)?;
                    let attribute = EnvironmentAttributeInfo::by_name(&name).ok_or_else(|| format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:environment_attribute]: {name}"))?;
                    if attribute.ty.to_float(attribute.default_value()).is_none() {
                        return Err(format!("{name} cannot be converted to a float"));
                    }
                    Ok(attribute)
                });
                record.build(|| Some(ContextFloatProvider::EnvironmentAttributeValue(attribute?)))
            }
            "minecraft:floor" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::Floor(input?)))
            }
            "minecraft:from_int" => {
                let input = record.required(tag, "input", ContextIntProvider::decode_holder_partial);
                record.build(|| Some(ContextFloatProvider::FromInt(input?)))
            }
            "minecraft:length" => {
                let inputs = record.required(tag, "inputs", ContextFloatProvider::decode_holders);
                record.build(|| Some(ContextFloatProvider::Length(inputs?)))
            }
            "minecraft:max" => {
                let inputs = record.required(tag, "inputs", ContextFloatProvider::decode_holders);
                record.build(|| Some(ContextFloatProvider::Maximum(inputs?)))
            }
            "minecraft:min" => {
                let inputs = record.required(tag, "inputs", ContextFloatProvider::decode_holders);
                record.build(|| Some(ContextFloatProvider::Minimum(inputs?)))
            }
            "minecraft:mod" => {
                let left = holder(&mut record, "left");
                let right = holder(&mut record, "right");
                record.build(|| Some(ContextFloatProvider::Modulus { left: left?, right: right? }))
            }
            "minecraft:negate" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::Negate(input?)))
            }
            "minecraft:number_dispatcher" => {
                let cases = record.required(tag, "cases", |t| codec::list_partial(t, |c| Case::decode_partial(c, ContextFloatProvider::decode_holder_partial)));
                let default_value = record.with_default(tag, "default", exactly(0.0), ContextFloatProvider::decode_holder_partial);
                record.build(|| Some(ContextFloatProvider::NumberDispatcher { cases: cases?, default_value }))
            }
            "minecraft:pow" => {
                let base = holder(&mut record, "base");
                let exponent = holder(&mut record, "exponent");
                record.build(|| Some(ContextFloatProvider::Power { base: base?, exponent: exponent? }))
            }
            "minecraft:mul" => {
                let inputs = record.required(tag, "inputs", ContextFloatProvider::decode_holders);
                record.build(|| Some(ContextFloatProvider::Product(inputs?)))
            }
            "minecraft:div" => {
                let left = holder(&mut record, "left");
                let right = holder(&mut record, "right");
                record.build(|| Some(ContextFloatProvider::Quotient { left: left?, right: right? }))
            }
            "minecraft:round" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::Round(input?)))
            }
            "minecraft:sin" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::Sine(input?)))
            }
            "minecraft:sqrt" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::SquareRoot(input?)))
            }
            "minecraft:storage" => {
                let access = record.absorb(StoredNumberAccess::decode_partial(tag));
                let fallback = fallback(&mut record);
                record.build(|| Some(ContextFloatProvider::StorageValue { access: access?, fallback }))
            }
            "minecraft:add" => {
                let inputs = record.required(tag, "inputs", ContextFloatProvider::decode_holders);
                record.build(|| Some(ContextFloatProvider::Sum(inputs?)))
            }
            "minecraft:truncate" => {
                let input = holder(&mut record, "input");
                record.build(|| Some(ContextFloatProvider::Truncate(input?)))
            }
            "minecraft:uniform" => {
                let min = holder(&mut record, "min");
                let max = holder(&mut record, "max");
                record.build(|| Some(ContextFloatProvider::UniformGenerator { min: min?, max: max? }))
            }
            "minecraft:weighted_list" => {
                let distribution = record.required(tag, "distribution", |t| decode_weighted_list(t, ContextFloatProvider::decode_holder_partial));
                record.build(|| Some(ContextFloatProvider::WeightedListValue(distribution?)))
            }
            other => DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:context_float_provider_type]: {other}")),
        }
    }

    pub fn get_float(&self, context: &mut LootContext) -> f32 {
        match self.get_float_unsafe(context) {
            Ok(result) if result.is_finite() => result,
            _ => 0.0,
        }
    }

    pub fn get_float_or_throw(&self, context: &mut LootContext) -> ArithmeticResult<f32> {
        let value = self.get_float_unsafe(context)?;
        if value.is_finite() { Ok(value) } else { Err(ArithmeticException::new(format!("Invalid value: {}", crate::nbt::java_float_to_string(value)))) }
    }

    pub fn get_float_unsafe(&self, context: &mut LootContext) -> ArithmeticResult<f32> {
        Ok(match self {
            ContextFloatProvider::Absolute(input) => input.value().get_float_unsafe(context)?.abs(),
            ContextFloatProvider::Average(inputs) => {
                let (mut sum, mut count) = (0.0f32, 0);
                for input in inputs {
                    sum += input.value().get_float_unsafe(context)?;
                    count += 1;
                }
                sum / count as f32
            }
            ContextFloatProvider::Ceiling(input) => mth::ceil(input.value().get_float_unsafe(context)? as f64) as f32,
            ContextFloatProvider::ConditionalValue { condition, on_true, on_false } => {
                let value = if condition.value().test(context) { on_true } else { on_false };
                value.value().get_float_unsafe(context)?
            }
            ContextFloatProvider::ConstantValue(value) => *value,
            ContextFloatProvider::Cosine(input) => mth::cos(input.value().get_float_unsafe(context)? as f64),
            ContextFloatProvider::Difference { left, right } => left.value().get_float_unsafe(context)? - right.value().get_float_unsafe(context)?,
            ContextFloatProvider::EnchantmentLevelProvider(amount) => amount.calculate(context.params.enchantment_level.unwrap_or(0)),
            ContextFloatProvider::EnvironmentAttributeValue(attribute) => {
                let value = context.environment_attribute_value(attribute);
                attribute.ty.to_float(&value).unwrap_or(0.0)
            }
            ContextFloatProvider::Floor(input) => input.value().get_float_unsafe(context)?.floor(),
            ContextFloatProvider::FromInt(input) => input.value().get_int_unsafe(context)? as f32,
            ContextFloatProvider::Length(inputs) => {
                let mut sum_of_squares = 0.0f32;
                for input in inputs {
                    let value = input.value().get_float_unsafe(context)?;
                    sum_of_squares += value * value;
                }
                mth::sqrt(sum_of_squares)
            }
            ContextFloatProvider::Maximum(inputs) => {
                let mut value = -f32::MAX;
                for input in inputs {
                    value = java_max_f(value, input.value().get_float_unsafe(context)?);
                }
                value
            }
            ContextFloatProvider::Minimum(inputs) => {
                let mut value = f32::MAX;
                for input in inputs {
                    value = java_min_f(value, input.value().get_float_unsafe(context)?);
                }
                value
            }
            ContextFloatProvider::Modulus { left, right } => {
                let right_value = right.value().get_float_unsafe(context)?;
                if right_value == 0.0 { f32::NAN } else { left.value().get_float_unsafe(context)? % right_value }
            }
            ContextFloatProvider::Negate(input) => -input.value().get_float_unsafe(context)?,
            ContextFloatProvider::NumberDispatcher { cases, default_value } => {
                let mut selected = default_value;
                for case in cases {
                    if case.test(context) {
                        selected = &case.value;
                        break;
                    }
                }
                selected.value().get_float_unsafe(context)?
            }
            ContextFloatProvider::Power { base, exponent } => {
                let (base, exponent) = (base.value().get_float_unsafe(context)?, exponent.value().get_float_unsafe(context)?);
                if base == 0.0 && exponent == 0.0 { f32::NAN } else { (base as f64).powf(exponent as f64) as f32 }
            }
            ContextFloatProvider::Product(inputs) => {
                let mut value = 1.0f32;
                for input in inputs {
                    value *= input.value().get_float_unsafe(context)?;
                }
                value
            }
            ContextFloatProvider::Quotient { left, right } => left.value().get_float_unsafe(context)? / right.value().get_float_unsafe(context)?,
            ContextFloatProvider::Round(input) => crate::util::java_round_f(input.value().get_float_unsafe(context)?) as f32,
            ContextFloatProvider::Sine(input) => mth::sin(input.value().get_float_unsafe(context)? as f64),
            ContextFloatProvider::SquareRoot(input) => mth::sqrt(input.value().get_float_unsafe(context)?),
            ContextFloatProvider::StorageValue { access, fallback } => match access.get_numeric_tag(context) {
                Some(value) => crate::world::item::component::codec::as_f32(&value),
                None => fallback.value().get_float_unsafe(context)?,
            },
            ContextFloatProvider::Sum(inputs) => {
                let mut value = 0.0f32;
                for input in inputs {
                    value += input.value().get_float_unsafe(context)?;
                }
                value
            }
            ContextFloatProvider::Truncate(input) => {
                let value = input.value().get_float_unsafe(context)?;
                if value > 0.0 { value.floor() } else { value.ceil() }
            }
            ContextFloatProvider::UniformGenerator { min, max } => {
                let (min, max) = (min.value().get_float_unsafe(context)?, max.value().get_float_unsafe(context)?);
                mth::next_float(&mut context.random(),min, max)
            }
            ContextFloatProvider::WeightedListValue(distribution) => get_random(distribution, context).value().get_float_unsafe(context)?,
        })
    }
}

impl Validatable for ContextFloatProvider {
    fn validate(&self, context: &ValidationContext) {
        match self {
            ContextFloatProvider::Absolute(input)
            | ContextFloatProvider::Ceiling(input)
            | ContextFloatProvider::Cosine(input)
            | ContextFloatProvider::Floor(input)
            | ContextFloatProvider::Negate(input)
            | ContextFloatProvider::Round(input)
            | ContextFloatProvider::Sine(input)
            | ContextFloatProvider::SquareRoot(input)
            | ContextFloatProvider::Truncate(input) => validatable::validate_holder(context, "input", input),
            ContextFloatProvider::FromInt(input) => validatable::validate_holder(context, "input", input),
            ContextFloatProvider::Average(inputs)
            | ContextFloatProvider::Length(inputs)
            | ContextFloatProvider::Maximum(inputs)
            | ContextFloatProvider::Minimum(inputs)
            | ContextFloatProvider::Product(inputs)
            | ContextFloatProvider::Sum(inputs) => validatable::validate_holder_set_with_min_size(context, "inputs", inputs, 1),
            ContextFloatProvider::ConditionalValue { condition, on_true, on_false } => {
                validatable::validate_holder(context, "condition", condition);
                validatable::validate_holder(context, "on_true", on_true);
                validatable::validate_holder(context, "on_false", on_false);
            }
            ContextFloatProvider::ConstantValue(_) => {}
            ContextFloatProvider::Difference { left, right } | ContextFloatProvider::Modulus { left, right } | ContextFloatProvider::Quotient { left, right } => {
                validatable::validate_holder(context, "left", left);
                validatable::validate_holder(context, "right", right);
            }
            ContextFloatProvider::EnchantmentLevelProvider(_) => context.validate_context_usage(&[LootContextParam::EnchantmentLevel]),
            ContextFloatProvider::EnvironmentAttributeValue(attribute) => {
                context.validate_context_usage(if attribute.is_positional { &[LootContextParam::Origin] } else { &[] })
            }
            ContextFloatProvider::NumberDispatcher { cases, default_value } => {
                validatable::validate_list(context, "cases", cases);
                validatable::validate_holder(context, "default", default_value);
            }
            ContextFloatProvider::Power { base, exponent } => {
                validatable::validate_holder(context, "base", base);
                validatable::validate_holder(context, "exponent", exponent);
            }
            ContextFloatProvider::StorageValue { fallback, .. } => validatable::validate_holder(context, "fallback", fallback),
            ContextFloatProvider::UniformGenerator { min, max } => {
                validatable::validate_holder(context, "min", min);
                validatable::validate_holder(context, "max", max);
            }
            ContextFloatProvider::WeightedListValue(distribution) => {
                let values: Vec<&Holder<ContextFloatProvider>> = distribution.iter().map(|(value, _)| value).collect();
                validatable::validate_holder_refs(context, "distribution", &values);
            }
        }
    }
}
