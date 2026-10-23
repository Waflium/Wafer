use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use super::dsl::ConstType;
use super::dynamic::{DataResult, Dynamic, get_number_value, get_stream, java_map_order, put, snbt};
use super::types::Type;

#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    Unit,
    Dynamic(Dynamic),
    Prim(Nbt),
    Pair(Box<Value>, Box<Value>),
    Left(Box<Value>),
    Right(Box<Value>),
    List(Vec<Value>),
    CompoundList(Vec<(Value, Value)>),
    Choice(String, Box<Value>),
}

fn combine(first: Option<String>, second: Option<String>) -> Option<String> {
    match (first, second) {
        (Some(first), Some(second)) => Some(format!("{first}; {second}")),
        (first, second) => first.or(second),
    }
}

fn not_a_map(input: &Option<Nbt>) -> String {
    format!("Not a map: {}", snbt(input))
}

fn map_like(entries: &Option<Nbt>) -> String {
    format!("MapLike[{}]", snbt(entries))
}

pub fn decode_const(ty: ConstType, input: &Option<Nbt>) -> DataResult<Nbt> {
    let number = || match input {
        Some(tag) => get_number_value(tag),
        None => DataResult::error("Not a number"),
    };
    match ty {
        ConstType::Bool => number().map(|value| Nbt::Byte((value.double_value() != 0.0) as i8)),
        ConstType::Int => number().map(|value| Nbt::Int(value.int_value())),
        ConstType::Long => number().map(|value| Nbt::Long(value.long_value())),
        ConstType::Byte => number().map(|value| Nbt::Byte(value.byte_value())),
        ConstType::Short => number().map(|value| Nbt::Short(value.short_value())),
        ConstType::Float => number().map(|value| Nbt::Float(value.float_value())),
        ConstType::Double => number().map(|value| Nbt::Double(value.double_value())),
        ConstType::String => match input {
            Some(Nbt::String(value)) => DataResult::success(Nbt::String(value.clone())),
            _ => DataResult::error("Not a string"),
        },
        ConstType::NamespacedString => match input {
            Some(Nbt::String(value)) => DataResult::success(Nbt::String(crate::util::datafix::schemas::namespaced_schema::ensure_namespaced(value))),
            _ => DataResult::error("Not a string"),
        },
    }
}

pub fn decode(ty: &Arc<Type>, input: Option<Nbt>) -> DataResult<(Value, Option<Nbt>)> {
    match &**ty {
        Type::Const(kind) => decode_const(*kind, &input).map(|value| (Value::Prim(value), None)),
        Type::Remainder => DataResult::success((Value::Dynamic(Dynamic::of(input)), None)),
        Type::EmptyPart => match input {
            Some(Nbt::Compound(_)) => DataResult::success((Value::Unit, input)),
            other => DataResult::error(not_a_map(&other)),
        },
        Type::Named(_, element) => decode(element, input),
        Type::Hook(element, pre_read, _) => decode(element, pre_read(Dynamic::of(input)).value),
        Type::RecursivePoint(..) => decode(&ty.unfold(), input),
        Type::Field(name, element) => {
            let value = match &input {
                Some(Nbt::Compound(entries)) => match entries.iter().find(|(key, _)| key == &**name) {
                    Some((_, value)) => value.clone(),
                    None => return DataResult::error(format!("No key {name} in {}", map_like(&input))),
                },
                other => return DataResult::error(not_a_map(other)),
            };
            decode(element, Some(value)).map(|(value, _)| (value, input))
        }
        Type::Product(first, second) => decode(first, input).flat_map(|(first_value, rest)| {
            decode(second, rest).map(|(second_value, rest)| (Value::Pair(Box::new(first_value), Box::new(second_value)), rest))
        }),
        Type::Sum(first, second) => {
            let first_read = decode(first, input.clone()).map(|(value, rest)| (Value::Left(Box::new(value)), rest));
            if first_read.is_success() {
                return first_read;
            }
            let second_read = decode(second, input).map(|(value, rest)| (Value::Right(Box::new(value)), rest));
            if second_read.is_success() {
                second_read
            } else if first_read.value.is_some() {
                first_read
            } else if second_read.value.is_some() {
                second_read
            } else {
                DataResult::error(format!(
                    "Failed to parse either. First: {}; Second: {}",
                    first_read.error.unwrap_or_default(),
                    second_read.error.unwrap_or_default()
                ))
            }
        }
        Type::List(element) => {
            let Some(elements) = input.as_ref().and_then(get_stream) else { return DataResult::error(format!("Not a list: {}", snbt(&input))) };
            let mut values = Vec::new();
            let mut failed = Vec::new();
            let mut error = None;
            for item in elements {
                let result = decode(element, Some(item.clone()));
                if let Some(message) = result.error {
                    failed.push(item);
                    error = combine(Some(message), error);
                }
                if let Some((value, _)) = result.value {
                    values.push(value);
                }
            }
            DataResult { value: Some((Value::List(values), Some(Nbt::List(failed)))), error }
        }
        Type::CompoundList(key, element) => {
            let Some(Nbt::Compound(entries)) = &input else { return DataResult::error(not_a_map(&input)) };
            let mut values = Vec::new();
            let mut failed = Vec::new();
            let mut error = None;
            for (name, item) in java_map_order(entries) {
                let key_result = decode(key, Some(Nbt::String(name.clone())));
                let value_result = decode(element, Some(item.clone()));
                let entry_error = combine(value_result.error.clone(), key_result.error.clone());
                if entry_error.is_some() {
                    failed.push((name.clone(), item.clone()));
                }
                if let (Some((key_value, _)), Some((item_value, _))) = (key_result.value, value_result.value) {
                    values.push((key_value, item_value));
                }
                error = combine(entry_error, error);
            }
            DataResult { value: Some((Value::CompoundList(values), Some(Nbt::Compound(failed)))), error }
        }
        Type::TaggedChoice(choice) => {
            let key = match &input {
                Some(Nbt::Compound(entries)) => match entries.iter().find(|(key, _)| key == &*choice.name) {
                    Some((_, value)) => value.clone(),
                    None => return DataResult::error(format!("No key {} in {}", choice.name, map_like(&input))),
                },
                other => return DataResult::error(not_a_map(other)),
            };
            decode_const(choice.key_type, &Some(key)).flat_map(|key| {
                let Nbt::String(key) = key else { return DataResult::error("Unsupported key") };
                match choice.get(&key) {
                    Some(choice_type) => decode(choice_type, input.clone()).map(|(value, _)| (Value::Choice(key, Box::new(value)), input.clone())),
                    None => DataResult::error(format!("Unsupported key: {key}")),
                }
            })
        }
    }
}

fn merge_to_map(prefix: Option<Nbt>, values: Vec<(String, Nbt)>) -> DataResult<Option<Nbt>> {
    match prefix {
        None => DataResult::success(Some(Nbt::Compound(values))),
        Some(Nbt::Compound(mut entries)) => {
            for (key, value) in values {
                put(&mut entries, &key, value);
            }
            DataResult::success(Some(Nbt::Compound(entries)))
        }
        Some(other) => DataResult { value: Some(Some(other.clone())), error: Some(format!("mergeToMap called with not a map: {}", crate::nbt::to_string(&other))) },
    }
}

fn merge_to_list(prefix: Option<Nbt>, values: Vec<Nbt>) -> DataResult<Option<Nbt>> {
    match prefix {
        None => DataResult::success(Some(Nbt::List(values))),
        Some(Nbt::List(mut list)) => {
            list.extend(values);
            DataResult::success(Some(Nbt::List(list)))
        }
        Some(other) => match get_stream(&other) {
            Some(mut list) => {
                if list.is_empty() {
                    return DataResult::success(Some(Nbt::List(values)));
                }
                let homogeneous = |values: &[Nbt], f: fn(&Nbt) -> bool| values.iter().all(f);
                match other {
                    Nbt::ByteArray(mut array) if homogeneous(&values, |v| matches!(v, Nbt::Byte(_))) => {
                        array.extend(values.iter().map(|v| if let Nbt::Byte(b) = v { *b } else { 0 }));
                        DataResult::success(Some(Nbt::ByteArray(array)))
                    }
                    Nbt::IntArray(mut array) if homogeneous(&values, |v| matches!(v, Nbt::Int(_))) => {
                        array.extend(values.iter().map(|v| if let Nbt::Int(i) = v { *i } else { 0 }));
                        DataResult::success(Some(Nbt::IntArray(array)))
                    }
                    Nbt::LongArray(mut array) if homogeneous(&values, |v| matches!(v, Nbt::Long(_))) => {
                        array.extend(values.iter().map(|v| if let Nbt::Long(l) = v { *l } else { 0 }));
                        DataResult::success(Some(Nbt::LongArray(array)))
                    }
                    _ => {
                        list.extend(values);
                        DataResult::success(Some(Nbt::List(list)))
                    }
                }
            }
            None => DataResult { value: Some(Some(other.clone())), error: Some(format!("mergeToList called with not a list: {}", crate::nbt::to_string(&other))) },
        },
    }
}

pub fn encode_start(ty: &Arc<Type>, value: &Value) -> DataResult<Option<Nbt>> {
    encode(ty, value, None)
}

pub fn encode(ty: &Arc<Type>, value: &Value, prefix: Option<Nbt>) -> DataResult<Option<Nbt>> {
    match (&**ty, value) {
        (Type::Const(_), Value::Prim(prim)) => match prefix {
            None => DataResult::success(Some(prim.clone())),
            Some(prefix) => DataResult {
                value: Some(Some(prim.clone())),
                error: Some(format!("Do not know how to append a primitive value {} to {}", crate::nbt::to_string(prim), crate::nbt::to_string(&prefix))),
            },
        },
        (Type::Remainder, Value::Dynamic(dynamic)) => match (&dynamic.value, prefix) {
            (None, prefix) => DataResult::success(prefix),
            (Some(casted), None) => DataResult::success(Some(casted.clone())),
            (Some(Nbt::Compound(entries)), prefix @ Some(Nbt::Compound(_))) => merge_to_map(prefix, entries.clone()),
            (Some(casted), Some(prefix)) => match get_stream(casted) {
                Some(values) if get_stream(&prefix).is_some() => merge_to_list(Some(prefix), values),
                _ => DataResult {
                    value: Some(Some(prefix.clone())),
                    error: Some(format!("Don't know how to merge {} and {}", crate::nbt::to_string(&prefix), crate::nbt::to_string(casted))),
                },
            },
        },
        (Type::EmptyPart, Value::Unit) => merge_to_map(prefix, Vec::new()),
        (Type::Named(_, element), value) => encode(element, value, prefix),
        (Type::RecursivePoint(..), value) => encode(&ty.unfold(), value, prefix),
        (Type::Hook(element, _, post_write), value) => encode(element, value, prefix).map(|result| post_write(Dynamic::of(result)).value),
        (Type::Field(name, element), value) => {
            let encoded = encode_start(element, value);
            let error = encoded.error.clone();
            let mut values = Vec::new();
            if let Some(Some(encoded)) = encoded.value {
                values.push((name.to_string(), encoded));
            }
            let built = merge_to_map(prefix, values);
            DataResult { value: built.value, error: combine(error, built.error) }
        }
        (Type::Product(first, second), Value::Pair(first_value, second_value)) => {
            encode(second, second_value, prefix).flat_map(|rest| encode(first, first_value, rest))
        }
        (Type::Sum(first, _), Value::Left(inner)) => encode(first, inner, prefix),
        (Type::Sum(_, second), Value::Right(inner)) => encode(second, inner, prefix),
        (Type::List(element), Value::List(values)) => {
            let mut encoded = Vec::new();
            let mut error = None;
            for value in values {
                let result = encode_start(element, value);
                error = combine(error, result.error);
                if let Some(Some(tag)) = result.value {
                    encoded.push(tag);
                }
            }
            let built = merge_to_list(prefix, encoded);
            DataResult { value: built.value, error: combine(error, built.error) }
        }
        (Type::CompoundList(key, element), Value::CompoundList(entries)) => {
            let mut encoded = Vec::new();
            let mut error = None;
            for (key_value, entry_value) in entries {
                let key_result = encode_start(key, key_value);
                let value_result = encode_start(element, entry_value);
                error = combine(error, combine(key_result.error, value_result.error));
                if let (Some(Some(Nbt::String(key))), Some(Some(value))) = (key_result.value, value_result.value) {
                    encoded.push((key, value));
                }
            }
            let built = merge_to_map(prefix, encoded);
            DataResult { value: built.value, error: combine(error, built.error) }
        }
        (Type::TaggedChoice(choice), Value::Choice(key, inner)) => {
            let Some(choice_type) = choice.get(key) else { return DataResult { value: Some(prefix), error: Some(format!("Unsupported key: {key}")) } };
            let encoded = encode_start(choice_type, inner);
            let mut values = Vec::new();
            let mut error = encoded.error.clone();
            match encoded.value {
                Some(Some(Nbt::Compound(entries))) => values.extend(entries),
                Some(other) => error = combine(error, Some(not_a_map(&other))),
                None => {}
            }
            put(&mut values, &choice.name, Nbt::String(key.clone()));
            let built = merge_to_map(prefix, values);
            DataResult { value: built.value, error: combine(error, built.error) }
        }
        (ty, value) => DataResult { value: Some(prefix), error: Some(format!("Value {value:?} doesn't match type {ty:?}")) },
    }
}

impl Value {
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::Prim(Nbt::String(value)) => Some(value),
            _ => None,
        }
    }

    pub fn map_string(self, f: impl FnOnce(&str) -> String) -> Value {
        match self {
            Value::Prim(Nbt::String(value)) => Value::Prim(Nbt::String(f(&value))),
            other => other,
        }
    }
}
