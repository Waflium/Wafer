use std::cell::Cell;

use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec as component_codec;

pub type DataResult<T> = Result<T, String>;

pub fn field<'a>(tag: &'a Nbt, name: &str) -> DataResult<&'a Nbt> {
    map(tag)?;
    tag.get(name).ok_or_else(|| format!("No key {name} in MapLike[{}]", crate::nbt::to_string(tag)))
}

pub fn no_key(key: &str, map: &serde_json::Value) -> String {
    format!("No key {key} in MapLike[{map}]")
}

pub fn record_errors(errors: &[Option<&String>]) -> DataResult<()> {
    let errors: Vec<&str> = errors.iter().rev().flatten().map(|e| e.as_str()).collect();
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

pub struct DecodeResult<T> {
    pub value: Option<T>,
    pub error: Option<String>,
}

impl<T> DecodeResult<T> {
    pub fn success(value: T) -> DecodeResult<T> {
        DecodeResult { value: Some(value), error: None }
    }

    pub fn error(message: impl Into<String>) -> DecodeResult<T> {
        DecodeResult { value: None, error: Some(message.into()) }
    }

    pub fn is_success(&self) -> bool {
        self.error.is_none()
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> DecodeResult<U> {
        DecodeResult { value: self.value.map(f), error: self.error }
    }

    pub fn flat_map<U>(self, f: impl FnOnce(T) -> DecodeResult<U>) -> DecodeResult<U> {
        let Some(value) = self.value else { return DecodeResult { value: None, error: self.error } };
        let second = f(value);
        let error = match (self.error, second.error) {
            (Some(first), Some(second)) => Some(format!("{first}; {second}")),
            (first, second) => first.or(second),
        };
        DecodeResult { value: second.value, error }
    }

    pub fn validate(self, validator: impl FnOnce(&T) -> DataResult<()>) -> DecodeResult<T> {
        self.flat_map(|value| match validator(&value) {
            Ok(()) => DecodeResult::success(value),
            Err(error) => DecodeResult::error(error),
        })
    }

    pub fn into_result(self) -> DataResult<T> {
        match self.error {
            None => Ok(self.value.expect("a successful value")),
            Some(message) => Err(message),
        }
    }

    pub fn into_decode_error(self) -> Result<T, DecodeError> {
        match self.error {
            None => Ok(self.value.expect("a successful value")),
            Some(message) => Err(DecodeError { message, partial: self.value.is_some() }),
        }
    }
}

impl<T> From<DataResult<T>> for DecodeResult<T> {
    fn from(result: DataResult<T>) -> DecodeResult<T> {
        match result {
            Ok(value) => DecodeResult::success(value),
            Err(message) => DecodeResult::error(message),
        }
    }
}

#[derive(Default)]
pub struct Record(Vec<String>);

impl Record {
    pub fn field<T>(&mut self, result: DataResult<T>) -> Option<T> {
        match result {
            Ok(value) => Some(value),
            Err(error) => {
                self.0.push(error);
                None
            }
        }
    }

    pub fn finish(self) -> DataResult<()> {
        if self.0.is_empty() { Ok(()) } else { Err(self.0.into_iter().rev().collect::<Vec<_>>().join("; ")) }
    }

    pub fn absorb<T>(&mut self, result: DecodeResult<T>) -> Option<T> {
        if let Some(error) = result.error {
            self.0.push(error);
        }
        result.value
    }

    pub fn required<T, R: Into<DecodeResult<T>>>(&mut self, tag: &Nbt, name: &str, decode: impl FnOnce(&Nbt) -> R) -> Option<T> {
        match field(tag, name) {
            Ok(value) => self.absorb(decode(value).into()),
            Err(error) => {
                self.0.push(error);
                None
            }
        }
    }

    pub fn optional<T, R: Into<DecodeResult<T>>>(&mut self, tag: &Nbt, name: &str, decode: impl FnOnce(&Nbt) -> R) -> Option<T> {
        let value = tag.get(name)?;
        self.absorb(decode(value).into())
    }

    pub fn with_default<T, R: Into<DecodeResult<T>>>(&mut self, tag: &Nbt, name: &str, default: T, decode: impl FnOnce(&Nbt) -> R) -> T {
        self.optional(tag, name, decode).unwrap_or(default)
    }

    pub fn build<T>(self, build: impl FnOnce() -> Option<T>) -> DecodeResult<T> {
        let error = if self.0.is_empty() { None } else { Some(self.0.into_iter().rev().collect::<Vec<_>>().join("; ")) };
        DecodeResult { value: build(), error }
    }
}

pub fn optional_field<'a>(tag: &'a Nbt, name: &str) -> Option<&'a Nbt> {
    tag.get(name)
}

pub fn decode_optional<T>(tag: &Nbt, name: &str, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<Option<T>> {
    optional_field(tag, name).map(decode).transpose()
}

pub fn decode_or<T>(tag: &Nbt, name: &str, default: T, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<T> {
    Ok(decode_optional(tag, name, decode)?.unwrap_or(default))
}

pub fn decode_field<T>(tag: &Nbt, name: &str, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<T> {
    decode(field(tag, name)?)
}

pub fn map(tag: &Nbt) -> DataResult<&[(String, Nbt)]> {
    component_codec::map(tag)
}

thread_local! {
    static NBT_OPS: Cell<bool> = const { Cell::new(false) };
}

pub fn with_nbt_ops<R>(decode: impl FnOnce() -> R) -> R {
    let previous = NBT_OPS.with(|nbt_ops| nbt_ops.replace(true));
    let result = decode();
    NBT_OPS.with(|nbt_ops| nbt_ops.set(previous));
    result
}

pub fn is_nbt_ops() -> bool {
    NBT_OPS.with(Cell::get)
}

pub fn is_json_value() -> bool {
    crate::nbt::is_json_source() && !is_nbt_ops()
}

pub fn number(tag: &Nbt) -> DataResult<&Nbt> {
    match tag {
        Nbt::Byte(_) if is_json_value() => Err(format!("Not a number: {}", crate::nbt::to_string(tag))),
        Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) | Nbt::LazilyParsedNumber(_) => Ok(tag),
        _ if is_nbt_ops() => Err("Not a number".to_string()),
        _ => Err(format!("Not a number: {}", crate::nbt::to_string(tag))),
    }
}

pub fn bool_value(tag: &Nbt) -> DataResult<bool> {
    if is_json_value() {
        return match tag {
            Nbt::Byte(value) => Ok(*value != 0),
            _ => Err(format!("Not a boolean: {}", crate::nbt::to_string(tag))),
        };
    }
    Ok(component_codec::as_f64(number(tag)?) != 0.0)
}

pub fn int(tag: &Nbt) -> DataResult<i32> {
    Ok(component_codec::as_i32(number(tag)?))
}

pub fn int_range(tag: &Nbt, min: i32, max: i32) -> DataResult<i32> {
    let value = int(tag)?;
    if value < min || value > max {
        return Err(format!("Value must be within range [{min};{max}]: {value}"));
    }
    Ok(value)
}

pub fn long(tag: &Nbt) -> DataResult<i64> {
    Ok(component_codec::as_i64(number(tag)?))
}

pub fn float(tag: &Nbt) -> DataResult<f32> {
    Ok(component_codec::as_f32(number(tag)?))
}

pub fn double(tag: &Nbt) -> DataResult<f64> {
    Ok(component_codec::as_f64(number(tag)?))
}

pub fn string(tag: &Nbt) -> DataResult<&str> {
    component_codec::string(tag)
}

pub fn identifier(tag: &Nbt) -> DataResult<String> {
    component_codec::identifier(tag)
}

pub fn byte_buffer(tag: &Nbt) -> DataResult<Vec<i8>> {
    let numbers: Vec<&Nbt> = match tag {
        Nbt::ByteArray(bytes) => return Ok(bytes.clone()),
        Nbt::List(items) => items.iter().collect(),
        Nbt::IntArray(ints) => return Ok(ints.iter().map(|&value| value as i8).collect()),
        Nbt::LongArray(longs) => return Ok(longs.iter().map(|&value| value as i8).collect()),
        _ => return Err("Not a list".to_string()),
    };
    if numbers.iter().all(|item| number(item).is_ok()) {
        Ok(numbers.iter().map(|item| component_codec::as_i32(item) as i8).collect())
    } else {
        Err(format!("Some elements are not bytes: {}", crate::nbt::to_string(tag)))
    }
}

pub fn list(tag: &Nbt) -> DataResult<Vec<Nbt>> {
    component_codec::list(tag)
}

pub fn list_of<T>(tag: &Nbt, decode: impl Fn(&Nbt) -> DataResult<T>) -> DataResult<Vec<T>> {
    let items = list(tag)?;
    let mut values = Vec::with_capacity(items.len());
    let mut errors = Vec::new();
    for item in &items {
        match decode(item) {
            Ok(value) => values.push(value),
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        return Ok(values);
    }
    errors.reverse();
    Err(errors.join("; "))
}

fn convert_json_to_nbt_ops(tag: &Nbt) -> Nbt {
    let narrowest = |value: i64| {
        if value as i8 as i64 == value {
            Nbt::Byte(value as i8)
        } else if value as i16 as i64 == value {
            Nbt::Short(value as i16)
        } else if value as i32 as i64 == value {
            Nbt::Int(value as i32)
        } else {
            Nbt::Long(value)
        }
    };
    match tag {
        Nbt::LazilyParsedNumber(text) => wafer_protocol::lazily_parsed_number::exact_nbt(text),
        Nbt::List(items) => Nbt::List(items.iter().map(convert_json_to_nbt_ops).collect()),
        Nbt::Compound(fields) => Nbt::Compound(fields.iter().map(|(name, value)| (name.clone(), convert_json_to_nbt_ops(value))).collect()),
        Nbt::Long(value) => narrowest(*value),
        Nbt::Double(value) if value.fract() == 0.0 && value.abs() < 9.2e18 => narrowest(*value as i64),
        Nbt::Double(value) if f64::from(*value as f32) == *value => Nbt::Float(*value as f32),
        other => other.clone(),
    }
}

fn contains_null(tag: &Nbt) -> bool {
    match tag {
        Nbt::End => true,
        Nbt::List(items) => items.iter().any(contains_null),
        Nbt::Compound(fields) => fields.iter().any(|(_, value)| contains_null(value)),
        _ => false,
    }
}

pub fn compound_tag(tag: &Nbt) -> DecodeResult<Nbt> {
    if contains_null(tag) {
        crate::core::data_registries::throw(
            "java.lang.NullPointerException",
            "Cannot invoke \"com.google.gson.JsonElement.getAsJsonPrimitive()\" because \"input\" is null".into(),
            &["com.mojang.serialization.JsonOps.convertTo(JsonOps.java:61)", "com.mojang.serialization.JsonOps.convertTo(JsonOps.java:25)"],
        );
    }
    match tag {
        Nbt::Compound(_) => DecodeResult::success(tag.clone()),
        other if is_json_value() => DecodeResult::error(format!("Not a compound tag: {}", crate::nbt::to_snbt(&convert_json_to_nbt_ops(other)))),
        other => DecodeResult::error(format!("Not a compound tag: {}", crate::nbt::to_string(other))),
    }
}

pub fn checked<T>(codec: &component_codec::Codec, tag: &Nbt, typed: impl Fn(&Nbt) -> DataResult<T>) -> DecodeResult<T> {
    let DecodeResult { value, error } = codec.decode_partial(tag);
    match error {
        None => DecodeResult::from(typed(tag)),
        Some(error) => DecodeResult { value: value.and_then(|partial| typed(&partial).ok()), error: Some(error) },
    }
}

pub fn compact_list_of<T>(tag: &Nbt, decode: impl Fn(&Nbt) -> DataResult<T>) -> DataResult<Vec<T>> {
    match tag {
        Nbt::List(_) | Nbt::ByteArray(_) | Nbt::IntArray(_) | Nbt::LongArray(_) => list_of(tag, decode),
        single => Ok(vec![decode(single)?]),
    }
}

pub fn either<A, B>(tag: &Nbt, first: impl FnOnce(&Nbt) -> DataResult<A>, second: impl FnOnce(&Nbt) -> DataResult<B>) -> DataResult<Result<A, B>> {
    match first(tag) {
        Ok(a) => Ok(Ok(a)),
        Err(first_error) => match second(tag) {
            Ok(b) => Ok(Err(b)),
            Err(second_error) => Err(format!("Failed to parse either. First: {first_error}; Second: {second_error}")),
        },
    }
}

pub struct DecodeError {
    pub message: String,
    pub partial: bool,
}

pub fn either_or_partial<A, B>(
    tag: &Nbt,
    first: impl FnOnce(&Nbt) -> DataResult<A>,
    second: impl FnOnce(&Nbt) -> Result<B, DecodeError>,
) -> DataResult<Result<A, B>> {
    match first(tag) {
        Ok(a) => Ok(Ok(a)),
        Err(first_error) => match second(tag) {
            Ok(b) => Ok(Err(b)),
            Err(DecodeError { message, partial: true }) => Err(message),
            Err(DecodeError { message, .. }) => Err(format!("Failed to parse either. First: {first_error}; Second: {message}")),
        },
    }
}

pub fn either_first_partial<A, B>(
    tag: &Nbt,
    first: impl FnOnce(&Nbt) -> Result<A, DecodeError>,
    second: impl FnOnce(&Nbt) -> DataResult<B>,
) -> DataResult<Result<A, B>> {
    match first(tag) {
        Ok(a) => Ok(Ok(a)),
        Err(first_error) => match second(tag) {
            Ok(b) => Ok(Err(b)),
            Err(_) if first_error.partial => Err(first_error.message),
            Err(second_error) => Err(format!("Failed to parse either. First: {}; Second: {second_error}", first_error.message)),
        },
    }
}

pub fn either_result<A, B>(first: DecodeResult<A>, second: impl FnOnce() -> DecodeResult<B>) -> DecodeResult<Result<A, B>> {
    if first.is_success() {
        return first.map(Ok);
    }
    let second = second();
    if second.is_success() {
        return second.map(Err);
    }
    if first.value.is_some() {
        return first.map(Ok);
    }
    if second.value.is_some() {
        return second.map(Err);
    }
    DecodeResult::error(format!(
        "Failed to parse either. First: {}; Second: {}",
        first.error.expect("an error"),
        second.error.expect("an error")
    ))
}

pub fn list_partial<T>(tag: &Nbt, decode: impl Fn(&Nbt) -> DecodeResult<T>) -> DecodeResult<Vec<T>> {
    sized_list_partial(tag, usize::MAX, decode)
}

pub fn sized_list_partial<T>(tag: &Nbt, max_size: usize, decode: impl Fn(&Nbt) -> DecodeResult<T>) -> DecodeResult<Vec<T>> {
    let items = match list(tag) {
        Ok(items) => items,
        Err(error) => return DecodeResult::error(error),
    };
    let mut elements = Vec::with_capacity(items.len().min(max_size));
    let mut errors: Vec<String> = Vec::new();
    for item in &items {
        if elements.len() >= max_size {
            continue;
        }
        let result = decode(item);
        if let Some(error) = result.error {
            errors.push(error);
        }
        if let Some(value) = result.value {
            elements.push(value);
        }
    }
    if items.len() > max_size {
        return DecodeResult { value: Some(elements), error: Some(format!("List is too long: {}, expected range [0-{max_size}]", items.len())) };
    }
    errors.reverse();
    DecodeResult { value: Some(elements), error: (!errors.is_empty()).then(|| errors.join("; ")) }
}

pub fn list_between_partial<T>(tag: &Nbt, min: usize, max: usize, decode: impl Fn(&Nbt) -> DecodeResult<T>) -> DecodeResult<Vec<T>> {
    let items = match list(tag) {
        Ok(items) => items,
        Err(error) => return DecodeResult::error(error),
    };
    let mut elements = Vec::with_capacity(items.len().min(max));
    let mut errors: Vec<String> = Vec::new();
    for item in &items {
        if elements.len() >= max {
            continue;
        }
        let result = decode(item);
        if let Some(error) = result.error {
            errors.push(error);
        }
        if let Some(value) = result.value {
            elements.push(value);
        }
    }
    if elements.len() < min {
        return DecodeResult::error(format!("List is too short: {}, expected range [{min}-{max}]", elements.len()));
    }
    if items.len() > max {
        return DecodeResult { value: Some(elements), error: Some(format!("List is too long: {}, expected range [{min}-{max}]", items.len())) };
    }
    errors.reverse();
    DecodeResult { value: Some(elements), error: (!errors.is_empty()).then(|| errors.join("; ")) }
}

pub fn holder_set_partial<T>(tag: &Nbt, registry: &str, element: impl Fn(&Nbt) -> DecodeResult<T>) -> DecodeResult<Vec<T>> {
    holder_set_partial_with(tag, registry, element, false)
}

pub fn holder_set_partial_with<T>(tag: &Nbt, registry: &str, element: impl Fn(&Nbt) -> DecodeResult<T>, always_use_list: bool) -> DecodeResult<Vec<T>> {
    let tag_error = match tag {
        Nbt::String(name) => match name.strip_prefix('#') {
            Some(location) => {
                return match crate::resources::identifier::read(location) {
                    Ok(_) if crate::core::data_registries::is_loading() && crate::server::reloadable_server_registries::is_reloadable(registry) => {
                        DecodeResult::error(format!("Registry {registry} is not available in this context"))
                    }
                    Ok(location) => DecodeResult::error(format!("Missing tag: '{location}' in '{registry}'")),
                    Err(message) => {
                        let compact = compact_list_partial(tag, element);
                        DecodeResult::error(format!("Failed to parse either. First: {message}; Second: {}", compact.error.expect("an error")))
                    }
                };
            }
            None => "Not a tag id".to_string(),
        },
        _ if is_json_value() => format!("Not a string: {}", crate::nbt::to_string(tag)),
        _ => "Not a string".to_string(),
    };
    let compact = if always_use_list { list_partial(tag, element) } else { compact_list_partial(tag, element) };
    if compact.value.is_some() {
        return compact;
    }
    DecodeResult::error(format!("Failed to parse either. First: {tag_error}; Second: {}", compact.error.expect("an error")))
}

pub fn compact_list_partial<T>(tag: &Nbt, element: impl Fn(&Nbt) -> DecodeResult<T>) -> DecodeResult<Vec<T>> {
    match tag {
        Nbt::List(_) | Nbt::ByteArray(_) | Nbt::IntArray(_) | Nbt::LongArray(_) => list_partial(tag, element),
        single => {
            let result = element(single);
            if result.value.is_some() {
                return result.map(|value| vec![value]);
            }
            let list_kind = if is_json_value() { "Not a json array" } else { "Not a list" };
            DecodeResult::error(format!("Failed to parse either. First: {list_kind}: {}; Second: {}", crate::nbt::to_string(single), result.error.expect("an error")))
        }
    }
}

pub fn holder_set<T>(tag: &Nbt, registry: &str, element: impl Fn(&Nbt) -> DataResult<T>) -> Result<Vec<T>, DecodeError> {
    holder_set_partial(tag, registry, |value| element(value).into()).into_decode_error()
}

pub fn string_enum<T: Copy>(tag: &Nbt, values: &[(&str, T)]) -> DataResult<T> {
    let name = string(tag)?;
    values.iter().find(|(n, _)| *n == name).map(|(_, v)| *v).ok_or_else(|| format!("Unknown element name:{name}"))
}

pub fn enum_by_name<T>(tag: &Nbt, by_name: impl FnOnce(&str) -> Option<T>) -> DataResult<T> {
    let name = string(tag)?;
    by_name(name).ok_or_else(|| format!("Unknown element name:{name}"))
}

pub fn legacy_enum<T>(tag: &Nbt, by_name: impl FnOnce(&str) -> Option<T>) -> DataResult<T> {
    let key = string(tag)?;
    by_name(key).ok_or_else(|| format!("No value with id: {key}"))
}

pub fn to_json(tag: &Nbt) -> serde_json::Value {
    use serde_json::Value;
    match tag {
        Nbt::End => Value::Null,
        Nbt::Byte(v) if crate::nbt::is_json_source() => Value::Bool(*v != 0),
        Nbt::Byte(v) => Value::from(*v),
        Nbt::Short(v) => Value::from(*v),
        Nbt::Int(v) => Value::from(*v),
        Nbt::Long(v) => Value::from(*v),
        Nbt::Float(v) => Value::from(*v),
        Nbt::Double(v) => Value::from(*v),
        Nbt::String(s) => Value::String(s.clone()),
        Nbt::ByteArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::IntArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::LongArray(a) => Value::Array(a.iter().map(|&v| Value::from(v)).collect()),
        Nbt::LazilyParsedNumber(text) => crate::util::gson::number_value(text),
        Nbt::List(items) => Value::Array(items.iter().map(to_json).collect()),
        Nbt::Compound(fields) => Value::Object(fields.iter().map(|(k, v)| (k.clone(), to_json(v))).collect()),
    }
}

pub fn unbounded_map<K, V>(tag: &Nbt, key: impl Fn(&str) -> DataResult<K>, value: impl Fn(&Nbt) -> DataResult<V>) -> DataResult<Vec<(K, V)>> {
    map(tag)?.iter().map(|(k, v)| Ok((key(k)?, value(v)?))).collect()
}

pub fn strict_unbounded_map<K: std::fmt::Display, V>(tag: &Nbt, key: impl Fn(&str) -> DataResult<K>, value: impl Fn(&Nbt) -> DataResult<V>) -> DataResult<Vec<(K, V)>> {
    let fields = map(tag)?;
    let mut entries = Vec::with_capacity(fields.len());
    for i in map_entry_order(fields) {
        let (name, element) = &fields[i];
        match (key(name), value(element)) {
            (Ok(key), Ok(value)) => entries.push((key, value)),
            (Ok(key), Err(error)) => return Err(format!("Map entry '{key}' : {error}")),
            (Err(error), Ok(_)) => return Err(error),
            (Err(key_error), Err(value_error)) => return Err(format!("{key_error}; {value_error}")),
        }
    }
    Ok(entries)
}

pub(crate) fn map_entry_order(fields: &[(String, Nbt)]) -> Vec<usize> {
    if crate::nbt::is_json_source() {
        return (0..fields.len()).collect();
    }
    wafer_protocol::nbt::java_hash_map_order(&fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>())
}

pub(crate) fn map_entry_failures(errors: Vec<String>, failed: Vec<(String, Nbt)>) -> String {
    let messages: Vec<String> = errors.into_iter().rev().collect();
    format!("{} missed input: {}", messages.join("; "), crate::nbt::to_string(&Nbt::Compound(failed.into())))
}

pub fn base_map<K, V>(tag: &Nbt, key: impl Fn(&str) -> DataResult<K>, value: impl Fn(&Nbt) -> DataResult<V>) -> DataResult<Vec<(K, V)>> {
    let fields = map(tag)?;
    let mut entries = Vec::with_capacity(fields.len());
    let mut errors = Vec::new();
    let mut failed = Vec::new();
    for i in map_entry_order(fields) {
        let (field_key, field_value) = &fields[i];
        match (key(field_key), value(field_value)) {
            (Ok(key), Ok(value)) => entries.push((key, value)),
            (key, value) => {
                errors.push([value.err(), key.err()].into_iter().flatten().collect::<Vec<_>>().join("; "));
                failed.push((field_key.clone(), field_value.clone()));
            }
        }
    }
    if errors.is_empty() { Ok(entries) } else { Err(map_entry_failures(errors, failed)) }
}

pub fn base_map_partial<K, V>(tag: &Nbt, key: impl Fn(&str) -> DataResult<K>, value: impl Fn(&Nbt) -> DecodeResult<V>) -> DecodeResult<Vec<(K, V)>> {
    let fields = match map(tag) {
        Ok(fields) => fields,
        Err(error) => return DecodeResult::error(error),
    };
    let mut entries = Vec::with_capacity(fields.len());
    let mut errors = Vec::new();
    let mut failed = Vec::new();
    for i in map_entry_order(fields) {
        let (field_key, field_value) = &fields[i];
        let key = key(field_key);
        let DecodeResult { value, error } = value(field_value);
        if error.is_some() || key.is_err() {
            errors.push([error, key.as_ref().err().cloned()].into_iter().flatten().collect::<Vec<_>>().join("; "));
            failed.push((field_key.clone(), field_value.clone()));
        }
        if let (Ok(key), Some(value)) = (key, value) {
            entries.push((key, value));
        }
    }
    let error = (!errors.is_empty()).then(|| map_entry_failures(errors, failed));
    DecodeResult { value: Some(entries), error }
}

pub fn dispatched_map<K, V>(tag: &Nbt, key: impl Fn(&str) -> DataResult<K>, value: impl FnMut(&K, &Nbt) -> DataResult<V>) -> DataResult<Vec<(K, V)>> {
    let DecodeResult { value, error } = dispatched_map_partial(tag, key, value);
    match error {
        Some(error) => Err(error),
        None => Ok(value.unwrap_or_default()),
    }
}

pub fn dispatched_map_partial<K, V>(tag: &Nbt, key: impl Fn(&str) -> DataResult<K>, mut value: impl FnMut(&K, &Nbt) -> DataResult<V>) -> DecodeResult<Vec<(K, V)>> {
    let fields = match map(tag) {
        Ok(fields) => fields,
        Err(error) => return DecodeResult::error(error),
    };
    let mut entries = Vec::with_capacity(fields.len());
    let mut errors = Vec::new();
    let mut failed = Vec::new();
    for i in map_entry_order(fields) {
        let (field_key, field_value) = &fields[i];
        match key(field_key) {
            Ok(key) => match value(&key, field_value) {
                Ok(value) => entries.push((key, value)),
                Err(error) => {
                    errors.push(error);
                    failed.push((field_key.clone(), field_value.clone()));
                }
            },
            Err(error) => {
                errors.push(format!("{error}; {error}"));
                failed.push((field_key.clone(), field_value.clone()));
            }
        }
    }
    DecodeResult { value: Some(entries), error: (!errors.is_empty()).then(|| map_entry_failures(errors, failed)) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(snbt: &str) -> Nbt {
        crate::nbt::parse_fully(snbt).expect("valid snbt")
    }

    fn int_element(tag: &Nbt) -> DecodeResult<i32> {
        DecodeResult::from(int(tag))
    }

    #[test]
    fn a_list_between_bounds_drops_failed_elements_and_checks_the_remaining_size() {
        let too_short = list_between_partial(&parse("[\"a\"]"), 1, 3, int_element);
        assert_eq!(too_short.error.as_deref(), Some("List is too short: 0, expected range [1-3]"));
        assert!(too_short.value.is_none());
        let too_long = list_between_partial(&parse("[1,2,3,4]"), 1, 3, int_element);
        assert_eq!(too_long.error.as_deref(), Some("List is too long: 4, expected range [1-3]"));
        assert_eq!(too_long.value, Some(vec![1, 2, 3]));
        let partial = list_between_partial(&parse("[1,\"a\",2]"), 1, 3, int_element);
        assert_eq!(partial.error.as_deref(), Some("Not a number: \"a\""));
        assert_eq!(partial.value, Some(vec![1, 2]));
    }

    #[test]
    fn a_strict_map_stops_at_the_first_failing_entry() {
        let key = |name: &str| -> DataResult<String> { Ok(name.to_string()) };
        assert_eq!(strict_unbounded_map(&parse("{a:1,b:\"x\",c:\"y\"}"), key, int).unwrap_err(), "Map entry 'b' : Not a number: \"x\"");
        let bad_key = |name: &str| -> DataResult<String> { if name == "a" { Err("bad key".to_string()) } else { Ok(name.to_string()) } };
        assert_eq!(strict_unbounded_map(&parse("{a:1}"), bad_key, int).unwrap_err(), "bad key");
        assert_eq!(strict_unbounded_map(&parse("{a:1,b:2}"), key, int).unwrap(), vec![("a".to_string(), 1), ("b".to_string(), 2)]);
    }

    #[test]
    fn a_legacy_enum_reports_unknown_names_like_the_legacy_string_codec() {
        let by_name = |name: &str| (name == "left_right").then_some(1);
        assert_eq!(legacy_enum(&parse("\"left_right\""), by_name), Ok(1));
        assert_eq!(legacy_enum(&parse("\"sideways\""), by_name), Err("No value with id: sideways".to_string()));
        assert!(legacy_enum(&parse("1"), by_name).is_err());
    }
}
