use std::sync::LazyLock;

use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::item::component::codec::{Codec, Dispatch, Selector, Stream};
use crate::world::item::component::types::{self, Pair, req};

fn codec_lazy() -> &'static Pair {
    static CODEC: LazyLock<Pair> = LazyLock::new(codec);
    &CODEC
}

pub fn codec() -> Pair {
    let value = |name: &'static str| req(name, (Codec::Lazy(codec_lazy), Stream::Lazy));
    let float = |name: &'static str| req(name, types::float(Codec::float()));
    let cases = vec![
        ("minecraft:clamped", vec![value("value"), float("min"), float("max")]),
        ("minecraft:fraction", vec![value("numerator"), value("denominator")]),
        ("minecraft:levels_squared", vec![float("added")]),
        ("minecraft:linear", vec![float("base"), float("per_level_above_first")]),
        ("minecraft:exponent", vec![value("base"), value("power")]),
        ("minecraft:lookup", vec![req("values", types::list(types::float(Codec::float()))), value("fallback")]),
    ];
    let dispatch = Codec::Dispatch(Box::new(Dispatch { key: "type", selector: Selector::Registry("minecraft:enchantment_level_based_value_type"), cases }));
    (Codec::Either(Box::new(Codec::float()), Box::new(dispatch)), Stream::Nbt)
}

#[derive(Clone, Debug, PartialEq)]
pub enum LevelBasedValue {
    Constant(f32),
    Clamped { value: Box<LevelBasedValue>, min: f32, max: f32 },
    Exponent { base: Box<LevelBasedValue>, power: Box<LevelBasedValue> },
    Fraction { numerator: Box<LevelBasedValue>, denominator: Box<LevelBasedValue> },
    LevelsSquared { added: f32 },
    Linear { base: f32, per_level_above_first: f32 },
    Lookup { values: Vec<f32>, fallback: Box<LevelBasedValue> },
}

impl LevelBasedValue {
    pub fn decode(tag: &Nbt) -> DataResult<LevelBasedValue> {
        LevelBasedValue::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<LevelBasedValue> {
        codec::either_result(DecodeResult::from(codec::float(tag)), || LevelBasedValue::decode_typed(tag)).map(|value| match value {
            Ok(constant) => LevelBasedValue::Constant(constant),
            Err(value) => value,
        })
    }

    fn decode_typed(tag: &Nbt) -> DecodeResult<LevelBasedValue> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let ty = match codec::decode_field(tag, "type", codec::identifier) {
            Ok(ty) => ty,
            Err(error) => return DecodeResult::error(error),
        };
        let mut record = Record::default();
        let boxed = |record: &mut Record, key: &str| record.required(tag, key, |t| LevelBasedValue::decode_partial(t).map(Box::new));
        match ty.as_str() {
            "minecraft:clamped" => {
                let value = boxed(&mut record, "value");
                let min = record.required(tag, "min", codec::float);
                let max = record.required(tag, "max", codec::float);
                record.build(|| Some(LevelBasedValue::Clamped { value: value?, min: min?, max: max? }))
            }
            "minecraft:exponent" => {
                let base = boxed(&mut record, "base");
                let power = boxed(&mut record, "power");
                record.build(|| Some(LevelBasedValue::Exponent { base: base?, power: power? }))
            }
            "minecraft:fraction" => {
                let numerator = boxed(&mut record, "numerator");
                let denominator = boxed(&mut record, "denominator");
                record.build(|| Some(LevelBasedValue::Fraction { numerator: numerator?, denominator: denominator? }))
            }
            "minecraft:levels_squared" => {
                let added = record.required(tag, "added", codec::float);
                record.build(|| Some(LevelBasedValue::LevelsSquared { added: added? }))
            }
            "minecraft:linear" => {
                let base = record.required(tag, "base", codec::float);
                let per_level_above_first = record.required(tag, "per_level_above_first", codec::float);
                record.build(|| Some(LevelBasedValue::Linear { base: base?, per_level_above_first: per_level_above_first? }))
            }
            "minecraft:lookup" => {
                let values = record.required(tag, "values", |t| codec::list_partial(t, |element| DecodeResult::from(codec::float(element))));
                let fallback = boxed(&mut record, "fallback");
                record.build(|| Some(LevelBasedValue::Lookup { values: values?, fallback: fallback? }))
            }
            other => DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_level_based_value_type]: {other}")),
        }
    }

    pub fn calculate(&self, level: i32) -> f32 {
        match self {
            LevelBasedValue::Constant(value) => *value,
            LevelBasedValue::Clamped { value, min, max } => wafer_core::mth::clamp_f(value.calculate(level), *min, *max),
            LevelBasedValue::Exponent { base, power } => (base.calculate(level) as f64).powf(power.calculate(level) as f64) as f32,
            LevelBasedValue::Fraction { numerator, denominator } => {
                let denominator = denominator.calculate(level);
                if denominator == 0.0 { 0.0 } else { numerator.calculate(level) / denominator }
            }
            LevelBasedValue::LevelsSquared { added } => level.wrapping_mul(level) as f32 + added,
            LevelBasedValue::Linear { base, per_level_above_first } => base + per_level_above_first * (level - 1) as f32,
            LevelBasedValue::Lookup { values, fallback } => values.get((level - 1) as usize).copied().unwrap_or_else(|| fallback.calculate(level)),
        }
    }
}
