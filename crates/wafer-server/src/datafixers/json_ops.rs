use serde_json::{Map, Number, Value};
use wafer_protocol::nbt::Nbt;

use super::dynamic::java_map_order;

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
