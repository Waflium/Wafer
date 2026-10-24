use serde_json::{Map, Number, Value};
use wafer_protocol::nbt::Nbt;

use super::dynamic::java_map_order;
use crate::nbt::{java_double_to_string, java_float_to_string};
use crate::util::gson::JsonElement;

pub fn convert_to_nbt(input: &Value) -> Option<Nbt> {
    match input {
        Value::Null => None,
        Value::Bool(value) => Some(Nbt::Byte(*value as i8)),
        Value::String(value) => Some(Nbt::String(value.clone())),
        Value::Number(number) => Some(convert_number(number)),
        Value::Array(values) => Some(Nbt::List(values.iter().filter_map(convert_to_nbt).collect())),
        Value::Object(fields) => Some(Nbt::Compound(fields.iter().filter_map(|(key, value)| convert_to_nbt(value).map(|value| (key.clone(), value))).collect())),
    }
}

fn convert_number(number: &Number) -> Nbt {
    let exact = match number.as_i64() {
        Some(value) => Some(value),
        None => number.as_f64().filter(|value| value.fract() == 0.0 && *value >= i64::MIN as f64 && *value < i64::MAX as f64 && number.as_u64().is_none()).map(|value| value as i64),
    };
    match exact {
        Some(value) if value as i8 as i64 == value => Nbt::Byte(value as i8),
        Some(value) if value as i16 as i64 == value => Nbt::Short(value as i16),
        Some(value) if value as i32 as i64 == value => Nbt::Int(value as i32),
        Some(value) => Nbt::Long(value),
        None => {
            let value = number.as_f64().unwrap_or(0.0);
            if value as f32 as f64 == value { Nbt::Float(value as f32) } else { Nbt::Double(value) }
        }
    }
}

pub fn convert_from_nbt(input: Option<&Nbt>) -> Value {
    let Some(input) = input else { return Value::Null };
    match input {
        Nbt::End => Value::Null,
        Nbt::Byte(value) => Value::from(*value),
        Nbt::Short(value) => Value::from(*value),
        Nbt::Int(value) => Value::from(*value),
        Nbt::Long(value) => Value::from(*value),
        Nbt::Float(value) => Value::from(*value as f64),
        Nbt::Double(value) => Value::from(*value),
        Nbt::String(value) => Value::String(value.clone()),
        Nbt::ByteArray(values) => Value::Array(values.iter().map(|&value| Value::from(value)).collect()),
        Nbt::IntArray(values) => Value::Array(values.iter().map(|&value| Value::from(value)).collect()),
        Nbt::LongArray(values) => Value::Array(values.iter().map(|&value| Value::from(value)).collect()),
        Nbt::List(values) => Value::Array(values.iter().map(|value| convert_from_nbt(Some(value))).collect()),
        Nbt::Compound(entries) => {
            let mut fields = Map::new();
            for (key, value) in java_map_order(entries) {
                fields.insert(key.clone(), convert_from_nbt(Some(value)));
            }
            Value::Object(fields)
        }
    }
}

pub fn convert_element_to_nbt(input: &JsonElement) -> Result<Option<Nbt>, String> {
    Ok(match input {
        JsonElement::Object(members) => {
            let mut entries: Vec<(String, Nbt)> = Vec::new();
            for (name, value) in members {
                match convert_element_to_nbt(value)? {
                    Some(value) => entries.push((name.clone(), value)),
                    None => return Err(format!("Unsupported null value for key {name}")),
                }
            }
            Some(Nbt::Compound(entries))
        }
        JsonElement::Array(elements) => {
            let mut list = Vec::new();
            for element in elements {
                match convert_element_to_nbt(element)? {
                    Some(value) => list.push(value),
                    None => return Err("Unsupported null list element".to_string()),
                }
            }
            Some(Nbt::List(list))
        }
        JsonElement::Null => None,
        JsonElement::String(value) => Some(Nbt::String(value.clone())),
        JsonElement::Boolean(value) => Some(Nbt::Byte(*value as i8)),
        JsonElement::Number(text) => Some(match long_value_exact(text)? {
            Some(l) => {
                if l as i8 as i64 == l {
                    Nbt::Byte(l as i8)
                } else if l as i16 as i64 == l {
                    Nbt::Short(l as i16)
                } else if l as i32 as i64 == l {
                    Nbt::Int(l as i32)
                } else {
                    Nbt::Long(l)
                }
            }
            None => {
                let d = text.parse::<f64>().map_err(|_| format!("NumberFormatException: {text}"))?;
                if d as f32 as f64 == d { Nbt::Float(d as f32) } else { Nbt::Double(d) }
            }
        }),
    })
}

fn long_value_exact(text: &str) -> Result<Option<i64>, String> {
    let bytes = text.as_bytes();
    let mut index = 0;
    let negative = bytes.first() == Some(&b'-');
    if negative || bytes.first() == Some(&b'+') {
        index += 1;
    }
    let mut digits: Vec<u8> = Vec::new();
    let mut fraction_length: i64 = 0;
    let mut seen_point = false;
    while index < bytes.len() {
        match bytes[index] {
            b'.' => seen_point = true,
            b'e' | b'E' => break,
            digit => {
                digits.push(digit - b'0');
                if seen_point {
                    fraction_length += 1;
                }
            }
        }
        index += 1;
    }
    let mut exponent: i64 = 0;
    if index < bytes.len() {
        exponent = text[index + 1..].trim_start_matches('+').parse::<i32>().map_err(|_| "NumberFormatException: Exponent overflow.".to_string())? as i64;
    }
    let scale = fraction_length - exponent;
    if scale > i32::MAX as i64 || scale < i32::MIN as i64 {
        return Err("NumberFormatException: Scale out of range.".to_string());
    }
    let Some(first_nonzero) = digits.iter().position(|&digit| digit != 0) else { return Ok(Some(0)) };
    let digits = &digits[first_nonzero..];
    let integer_digits: &[u8] = if scale <= 0 {
        digits
    } else if scale as usize >= digits.len() {
        return Ok(None);
    } else {
        let split = digits.len() - scale as usize;
        if digits[split..].iter().any(|&digit| digit != 0) {
            return Ok(None);
        }
        &digits[..split]
    };
    let trailing_zeros = if scale < 0 { -scale } else { 0 };
    if integer_digits.len() as i64 + trailing_zeros > 19 {
        return Ok(None);
    }
    let mut value: i128 = 0;
    for &digit in integer_digits {
        value = value * 10 + digit as i128;
    }
    for _ in 0..trailing_zeros {
        value *= 10;
    }
    if negative {
        value = -value;
    }
    Ok(i64::try_from(value).ok())
}

pub fn convert_nbt_to_element(input: &Nbt) -> JsonElement {
    match input {
        Nbt::End => JsonElement::Null,
        Nbt::Byte(value) => JsonElement::Number(value.to_string()),
        Nbt::Short(value) => JsonElement::Number(value.to_string()),
        Nbt::Int(value) => JsonElement::Number(value.to_string()),
        Nbt::Long(value) => JsonElement::Number(value.to_string()),
        Nbt::Float(value) => JsonElement::Number(java_float_to_string(*value)),
        Nbt::Double(value) => JsonElement::Number(java_double_to_string(*value)),
        Nbt::ByteArray(values) => JsonElement::Array(values.iter().map(|value| JsonElement::Number(value.to_string())).collect()),
        Nbt::String(value) => JsonElement::String(value.clone()),
        Nbt::List(values) => JsonElement::Array(values.iter().map(convert_nbt_to_element).collect()),
        Nbt::Compound(entries) => JsonElement::Object(entries.iter().map(|(name, value)| (name.clone(), convert_nbt_to_element(value))).collect()),
        Nbt::IntArray(values) => JsonElement::Array(values.iter().map(|value| JsonElement::Number(value.to_string())).collect()),
        Nbt::LongArray(values) => JsonElement::Array(values.iter().map(|value| JsonElement::Number(value.to_string())).collect()),
    }
}
