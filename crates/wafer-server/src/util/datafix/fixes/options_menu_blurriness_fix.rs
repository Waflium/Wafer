use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::OPTIONS;

pub struct OptionsMenuBlurrinessFix {
    base: DataFixBase,
}

impl OptionsMenuBlurrinessFix {
    pub fn new(output_schema: &Arc<Schema>) -> OptionsMenuBlurrinessFix {
        OptionsMenuBlurrinessFix { base: DataFixBase::new(output_schema, false) }
    }
}

fn convert_to_int_range(float_blurriness: &str) -> i32 {
    match java_parse_float(float_blurriness) {
        Some(blurriness) => java_round(blurriness * 10.0),
        None => 5,
    }
}

fn java_round(value: f32) -> i32 {
    (value as f64 + 0.5).floor() as i32
}

fn java_parse_float(input: &str) -> Option<f32> {
    let trimmed = input.trim_matches(|c: char| c <= ' ');
    let (negative, unsigned) = match trimmed.as_bytes().first() {
        Some(b'-') => (true, &trimmed[1..]),
        Some(b'+') => (false, &trimmed[1..]),
        _ => (false, trimmed),
    };
    let magnitude = match unsigned {
        "NaN" => f32::NAN,
        "Infinity" => f32::INFINITY,
        _ => parse_float_literal(unsigned)?,
    };
    Some(if negative { -magnitude } else { magnitude })
}

fn parse_float_literal(unsigned: &str) -> Option<f32> {
    if let Some(hex) = unsigned.strip_prefix("0x").or_else(|| unsigned.strip_prefix("0X")) {
        return parse_hex_float(hex);
    }
    let literal = unsigned.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(unsigned);
    let bytes = literal.as_bytes();
    let mut index = 0;
    let mut digits = 0;
    while index < bytes.len() && bytes[index].is_ascii_digit() {
        index += 1;
        digits += 1;
    }
    if index < bytes.len() && bytes[index] == b'.' {
        index += 1;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
            digits += 1;
        }
    }
    if digits == 0 {
        return None;
    }
    if index < bytes.len() && (bytes[index] == b'e' || bytes[index] == b'E') {
        index += 1;
        if index < bytes.len() && (bytes[index] == b'+' || bytes[index] == b'-') {
            index += 1;
        }
        let exponent_start = index;
        while index < bytes.len() && bytes[index].is_ascii_digit() {
            index += 1;
        }
        if index == exponent_start {
            return None;
        }
    }
    if index != bytes.len() {
        return None;
    }
    literal.parse::<f32>().ok()
}

fn parse_hex_float(hex: &str) -> Option<f32> {
    let literal = hex.strip_suffix(['f', 'F', 'd', 'D']).unwrap_or(hex);
    let (mantissa, exponent) = literal.split_once(['p', 'P'])?;
    let (integer, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    if integer.is_empty() && fraction.is_empty() {
        return None;
    }
    let mut value = 0.0f64;
    for digit in integer.chars() {
        value = value * 16.0 + digit.to_digit(16)? as f64;
    }
    let mut scale = 1.0f64 / 16.0;
    for digit in fraction.chars() {
        value += digit.to_digit(16)? as f64 * scale;
        scale /= 16.0;
    }
    let exponent_digits = exponent.strip_prefix(['+', '-']).unwrap_or(exponent);
    if exponent_digits.is_empty() || !exponent_digits.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let exponent: i32 = exponent.parse().ok()?;
    Some((value * 2.0f64.powi(exponent)) as f32)
}

impl DataFix for OptionsMenuBlurrinessFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("OptionsMenuBlurrinessFix", self.get_input_schema().get_type(OPTIONS), |input| {
            input.update_remainder(|tag| {
                tag.update("menuBackgroundBlurriness", |value| {
                    let int_value = convert_to_int_range(&value.as_string_or("0.5"));
                    value.create_string(int_value.to_string())
                })
            })
        })
    }
}
