use wafer_core::random::RandomSource;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::item::enchantment::level_based_value::LevelBasedValue;

#[derive(Clone, Debug, PartialEq)]
pub enum EnchantmentValueEffect {
    AddValue(LevelBasedValue),
    AllOf(Vec<EnchantmentValueEffect>),
    MultiplyValue(LevelBasedValue),
    RemoveBinomial(LevelBasedValue),
    ScaleExponentially { base: LevelBasedValue, exponent: LevelBasedValue },
    SetValue(LevelBasedValue),
}

impl EnchantmentValueEffect {
    pub fn decode(tag: &Nbt) -> DataResult<EnchantmentValueEffect> {
        EnchantmentValueEffect::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<EnchantmentValueEffect> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let ty = match codec::decode_field(tag, "type", codec::identifier) {
            Ok(ty) => ty,
            Err(error) => return DecodeResult::error(error),
        };
        let mut record = Record::default();
        let value = |record: &mut Record, key: &str| record.required(tag, key, LevelBasedValue::decode_partial);
        match ty.as_str() {
            "minecraft:add" => {
                let value = value(&mut record, "value");
                record.build(|| Some(EnchantmentValueEffect::AddValue(value?)))
            }
            "minecraft:all_of" => {
                let effects = record.required(tag, "effects", |t| codec::list_partial(t, EnchantmentValueEffect::decode_partial));
                record.build(|| Some(EnchantmentValueEffect::AllOf(effects?)))
            }
            "minecraft:multiply" => {
                let factor = value(&mut record, "factor");
                record.build(|| Some(EnchantmentValueEffect::MultiplyValue(factor?)))
            }
            "minecraft:remove_binomial" => {
                let chance = value(&mut record, "chance");
                record.build(|| Some(EnchantmentValueEffect::RemoveBinomial(chance?)))
            }
            "minecraft:exponential" => {
                let base = value(&mut record, "base");
                let exponent = value(&mut record, "exponent");
                record.build(|| Some(EnchantmentValueEffect::ScaleExponentially { base: base?, exponent: exponent? }))
            }
            "minecraft:set" => {
                let value = value(&mut record, "value");
                record.build(|| Some(EnchantmentValueEffect::SetValue(value?)))
            }
            other => DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / minecraft:enchantment_value_effect_type]: {other}")),
        }
    }

    pub fn process(&self, enchantment_level: i32, random: &mut impl RandomSource, input_value: f32) -> f32 {
        match self {
            EnchantmentValueEffect::AddValue(value) => input_value + value.calculate(enchantment_level),
            EnchantmentValueEffect::AllOf(effects) => effects.iter().fold(input_value, |value, effect| effect.process(enchantment_level, random, value)),
            EnchantmentValueEffect::MultiplyValue(factor) => input_value * factor.calculate(enchantment_level),
            EnchantmentValueEffect::RemoveBinomial(chance) => {
                let (n, p) = (input_value, chance.calculate(enchantment_level));
                let mut drop = 0;
                if !(n <= 128.0) && !(n * p < 20.0) && !(n * (1.0 - p) < 20.0) {
                    let miu = ((n * p) as f64).floor();
                    let sigma = ((n * p * (1.0 - p)) as f64).sqrt();
                    drop = (miu + random.next_gaussian() * sigma + 0.5).floor() as i64 as i32;
                    drop = drop.clamp(0, n as i32);
                } else {
                    let mut y = 0;
                    while (y as f32) < n {
                        if random.next_float() < p {
                            drop += 1;
                        }
                        y += 1;
                    }
                }
                n - drop as f32
            }
            EnchantmentValueEffect::ScaleExponentially { base, exponent } => {
                (input_value as f64 * (base.calculate(enchantment_level) as f64).powf(exponent.calculate(enchantment_level) as f64)) as f32
            }
            EnchantmentValueEffect::SetValue(value) => value.calculate(enchantment_level),
        }
    }
}
