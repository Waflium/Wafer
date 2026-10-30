use wafer_protocol::nbt::{Nbt, TagMap};

use crate::util::codec::DecodeResult;

pub type DataResult<T> = DecodeResult<T>;

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Number {
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
}

impl Number {
    pub fn int_value(self) -> i32 {
        match self {
            Number::Byte(v) => v as i32,
            Number::Short(v) => v as i32,
            Number::Int(v) => v,
            Number::Long(v) => v as i32,
            Number::Float(v) => v as i32,
            Number::Double(v) => v as i32,
        }
    }

    pub fn long_value(self) -> i64 {
        match self {
            Number::Byte(v) => v as i64,
            Number::Short(v) => v as i64,
            Number::Int(v) => v as i64,
            Number::Long(v) => v,
            Number::Float(v) => v as i64,
            Number::Double(v) => v as i64,
        }
    }

    pub fn byte_value(self) -> i8 {
        match self {
            Number::Long(v) => v as i8,
            other => other.int_value() as i8,
        }
    }

    pub fn short_value(self) -> i16 {
        match self {
            Number::Long(v) => v as i16,
            other => other.int_value() as i16,
        }
    }

    pub fn float_value(self) -> f32 {
        match self {
            Number::Byte(v) => v as f32,
            Number::Short(v) => v as f32,
            Number::Int(v) => v as f32,
            Number::Long(v) => v as f32,
            Number::Float(v) => v,
            Number::Double(v) => v as f32,
        }
    }

    pub fn double_value(self) -> f64 {
        match self {
            Number::Byte(v) => v as f64,
            Number::Short(v) => v as f64,
            Number::Int(v) => v as f64,
            Number::Long(v) => v as f64,
            Number::Float(v) => v as f64,
            Number::Double(v) => v,
        }
    }
}

pub fn get_number_value(tag: &Nbt) -> DataResult<Number> {
    match *tag {
        Nbt::Byte(v) => DataResult::success(Number::Byte(v)),
        Nbt::Short(v) => DataResult::success(Number::Short(v)),
        Nbt::Int(v) => DataResult::success(Number::Int(v)),
        Nbt::Long(v) => DataResult::success(Number::Long(v)),
        Nbt::Float(v) => DataResult::success(Number::Float(v)),
        Nbt::Double(v) => DataResult::success(Number::Double(v)),
        _ => DataResult::error("Not a number"),
    }
}

pub fn get_stream(tag: &Nbt) -> Option<Vec<Nbt>> {
    match tag {
        Nbt::List(list) => Some(list.clone()),
        Nbt::ByteArray(values) => Some(values.iter().map(|&v| Nbt::Byte(v)).collect()),
        Nbt::IntArray(values) => Some(values.iter().map(|&v| Nbt::Int(v)).collect()),
        Nbt::LongArray(values) => Some(values.iter().map(|&v| Nbt::Long(v)).collect()),
        _ => None,
    }
}

pub fn merge_to_map_like(prefix: Option<TagMap>, values: &TagMap) -> TagMap {
    if values.is_empty() {
        return prefix.unwrap_or_default();
    }
    let mut output = prefix.map_or_else(TagMap::new, TagMap::shallow_copy);
    for (key, value) in values.iteration() {
        output.put(key, value.clone());
    }
    output
}

pub fn snbt(value: &Option<Nbt>) -> String {
    match value {
        Some(tag) => crate::nbt::to_string(tag),
        None => "END".to_string(),
    }
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Dynamic {
    pub value: Option<Nbt>,
}

impl Dynamic {
    pub fn new(value: Nbt) -> Dynamic {
        Dynamic::of(Some(value))
    }

    pub fn empty() -> Dynamic {
        Dynamic { value: None }
    }

    pub fn of(value: Option<Nbt>) -> Dynamic {
        Dynamic { value: value.filter(|value| !matches!(value, Nbt::End)) }
    }

    pub fn get_value(&self) -> Option<&Nbt> {
        self.value.as_ref()
    }

    pub fn into_value(self) -> Option<Nbt> {
        self.value
    }

    pub fn map(self, f: impl FnOnce(Option<Nbt>) -> Option<Nbt>) -> Dynamic {
        Dynamic { value: f(self.value) }
    }

    pub fn create_string(&self, value: impl Into<String>) -> Dynamic {
        Dynamic::new(Nbt::String(value.into()))
    }

    pub fn create_int(&self, value: i32) -> Dynamic {
        Dynamic::new(Nbt::Int(value))
    }

    pub fn create_long(&self, value: i64) -> Dynamic {
        Dynamic::new(Nbt::Long(value))
    }

    pub fn create_byte(&self, value: i8) -> Dynamic {
        Dynamic::new(Nbt::Byte(value))
    }

    pub fn create_short(&self, value: i16) -> Dynamic {
        Dynamic::new(Nbt::Short(value))
    }

    pub fn create_float(&self, value: f32) -> Dynamic {
        Dynamic::new(Nbt::Float(value))
    }

    pub fn create_double(&self, value: f64) -> Dynamic {
        Dynamic::new(Nbt::Double(value))
    }

    pub fn create_numeric(&self, value: Number) -> Dynamic {
        Dynamic::new(Nbt::Double(value.double_value()))
    }

    pub fn create_boolean(&self, value: bool) -> Dynamic {
        Dynamic::new(Nbt::Byte(value as i8))
    }

    pub fn empty_map(&self) -> Dynamic {
        Dynamic::new(Nbt::Compound(TagMap::new()))
    }

    pub fn empty_list(&self) -> Dynamic {
        Dynamic::new(Nbt::List(Vec::new()))
    }

    pub fn create_map(&self, entries: impl IntoIterator<Item = (Dynamic, Dynamic)>) -> Dynamic {
        let mut map = TagMap::new();
        for (key, value) in entries {
            let Some(Nbt::String(key)) = key.value else { panic!("Cannot create map with non-string key: {}", snbt(&key.value)) };
            map.put(&key, value.value.unwrap_or(Nbt::End));
        }
        Dynamic::new(Nbt::Compound(map))
    }

    pub fn create_list(&self, values: impl IntoIterator<Item = Dynamic>) -> Dynamic {
        Dynamic::new(Nbt::List(values.into_iter().filter_map(|value| value.value).collect()))
    }

    pub fn create_int_list(&self, values: impl IntoIterator<Item = i32>) -> Dynamic {
        Dynamic::new(Nbt::IntArray(values.into_iter().collect()))
    }

    pub fn create_long_list(&self, values: impl IntoIterator<Item = i64>) -> Dynamic {
        Dynamic::new(Nbt::LongArray(values.into_iter().collect()))
    }

    pub fn create_byte_list(&self, values: impl IntoIterator<Item = i8>) -> Dynamic {
        Dynamic::new(Nbt::ByteArray(values.into_iter().collect()))
    }

    pub fn get(&self, key: &str) -> OptionalDynamic {
        OptionalDynamic(match &self.value {
            Some(Nbt::Compound(entries)) => match entries.iter().find(|(k, _)| k == key) {
                Some((_, value)) => DataResult::success(Dynamic::new(value.clone())),
                None => DataResult::error(format!("key missing: {key} in {}", snbt(&self.value))),
            },
            other => DataResult::error(format!("Not a map: {}", snbt(other))),
        })
    }

    pub fn get_element(&self, key: &str) -> DataResult<Nbt> {
        match &self.value {
            Some(Nbt::Compound(entries)) => match entries.iter().find(|(k, _)| k == key) {
                Some((_, value)) => DataResult::success(value.clone()),
                None => DataResult::error(format!("No element {key} in the map")),
            },
            _ => DataResult::error(format!("Not a map: {}", snbt(&self.value))),
        }
    }

    pub fn remove(self, key: &str) -> Dynamic {
        match self.value {
            Some(Nbt::Compound(entries)) => {
                let mut result = entries.shallow_copy();
                result.retain(|(k, _)| k != key);
                Dynamic::new(Nbt::Compound(result))
            }
            other => Dynamic { value: other },
        }
    }

    pub fn set(self, key: &str, value: Dynamic) -> Dynamic {
        match self.value {
            Some(Nbt::Compound(entries)) => {
                let mut output = entries.shallow_copy();
                output.put(key, value.value.unwrap_or(Nbt::End));
                Dynamic::new(Nbt::Compound(output))
            }
            None => {
                let mut output = TagMap::new();
                output.put(key, value.value.unwrap_or(Nbt::End));
                Dynamic::new(Nbt::Compound(output))
            }
            other => Dynamic { value: other },
        }
    }

    pub fn update(self, key: &str, f: impl FnOnce(Dynamic) -> Dynamic) -> Dynamic {
        match self.get(key).result() {
            Some(value) => {
                let value = f(value);
                self.set(key, value)
            }
            None => self,
        }
    }

    pub fn set_field_if_present(self, field: &str, value: Option<Dynamic>) -> Dynamic {
        match value {
            Some(value) => self.set(field, value),
            None => self,
        }
    }

    pub fn rename_field(self, old_field_name: &str, new_field_name: &str) -> Dynamic {
        self.rename_and_fix_field(old_field_name, new_field_name, |value| value)
    }

    pub fn replace_field(self, old_field_name: &str, new_field_name: &str, new_value: Option<Dynamic>) -> Dynamic {
        self.remove(old_field_name).set_field_if_present(new_field_name, new_value)
    }

    pub fn rename_and_fix_field(self, old_field_name: &str, new_field_name: &str, fixer: impl FnOnce(Dynamic) -> Dynamic) -> Dynamic {
        let value = self.get(old_field_name).result().map(fixer);
        self.remove(old_field_name).set_field_if_present(new_field_name, value)
    }

    pub fn copy_field(source: &Dynamic, source_key: &str, target: Dynamic, target_key: &str) -> Dynamic {
        Dynamic::copy_and_fix_field(source, source_key, target, target_key, |value| value)
    }

    pub fn copy_and_fix_field(source: &Dynamic, source_key: &str, target: Dynamic, target_key: &str, fixer: impl FnOnce(Dynamic) -> Dynamic) -> Dynamic {
        match source.get(source_key).result() {
            Some(value) => target.set(target_key, fixer(value)),
            None => target,
        }
    }

    pub fn as_number(&self) -> DataResult<Number> {
        match &self.value {
            Some(tag) => get_number_value(tag),
            None => DataResult::error("Not a number"),
        }
    }

    pub fn as_string(&self) -> DataResult<String> {
        match &self.value {
            Some(Nbt::String(value)) => DataResult::success(value.clone()),
            _ => DataResult::error("Not a string"),
        }
    }

    pub fn as_boolean(&self) -> DataResult<bool> {
        self.as_number().map(|value| value.double_value() != 0.0)
    }

    pub fn as_stream_opt(&self) -> DataResult<Vec<Dynamic>> {
        match self.value.as_ref().and_then(get_stream) {
            Some(values) => DataResult::success(values.into_iter().map(Dynamic::new).collect()),
            None => DataResult::error("Not a list"),
        }
    }

    pub fn as_map_opt(&self) -> DataResult<Vec<(Dynamic, Dynamic)>> {
        match &self.value {
            Some(Nbt::Compound(entries)) => DataResult::success(
                entries.iteration().map(|(key, value)| (Dynamic::new(Nbt::String(key.clone())), Dynamic::new(value.clone()))).collect(),
            ),
            other => DataResult::error(format!("Not a map: {}", snbt(other))),
        }
    }

    pub fn get_map_values(&self) -> DataResult<Vec<(Dynamic, Dynamic)>> {
        self.as_map_opt()
    }

    pub fn update_map_values(self, mut updater: impl FnMut((Dynamic, Dynamic)) -> (Dynamic, Dynamic)) -> Dynamic {
        match self.get_map_values().value {
            Some(entries) if self.get_map_values().is_success() => {
                let mut updated: Vec<(Dynamic, Dynamic)> = Vec::with_capacity(entries.len());
                let mut positions: std::collections::HashMap<String, usize> = std::collections::HashMap::with_capacity(entries.len());
                for entry in entries {
                    let (key, value) = updater(entry);
                    if let Some(Nbt::String(name)) = &key.value {
                        if let Some(&position) = positions.get(name) {
                            panic!("Duplicate key NBT[{}] (attempted merging values NBT[{}] and NBT[{}])", snbt(&key.value), snbt(&updated[position].1.value), snbt(&value.value));
                        }
                        positions.insert(name.clone(), updated.len());
                    }
                    updated.push((key, value));
                }
                self.create_map(updated)
            }
            _ => self,
        }
    }

    pub fn as_int_stream_opt(&self) -> DataResult<Vec<i32>> {
        match &self.value {
            Some(Nbt::IntArray(values)) => DataResult::success(values.clone()),
            _ => self.as_stream_opt().flat_map(|values| {
                let numbers: Option<Vec<i32>> = values.iter().map(|value| value.as_number().value.map(Number::int_value)).collect();
                match numbers {
                    Some(numbers) if values.iter().all(|value| value.as_number().is_success()) => DataResult::success(numbers),
                    _ => DataResult::error("Some elements are not ints"),
                }
            }),
        }
    }

    pub fn as_long_stream_opt(&self) -> DataResult<Vec<i64>> {
        match &self.value {
            Some(Nbt::LongArray(values)) => DataResult::success(values.clone()),
            _ => self.as_stream_opt().flat_map(|values| {
                let numbers: Option<Vec<i64>> = values.iter().map(|value| value.as_number().value.map(Number::long_value)).collect();
                match numbers {
                    Some(numbers) if values.iter().all(|value| value.as_number().is_success()) => DataResult::success(numbers),
                    _ => DataResult::error("Some elements are not longs"),
                }
            }),
        }
    }

    pub fn as_byte_buffer_opt(&self) -> DataResult<Vec<i8>> {
        match &self.value {
            Some(Nbt::ByteArray(values)) => DataResult::success(values.clone()),
            _ => self.as_stream_opt().flat_map(|values| {
                let numbers: Option<Vec<i8>> = values.iter().map(|value| value.as_number().value.map(Number::byte_value)).collect();
                match numbers {
                    Some(numbers) if values.iter().all(|value| value.as_number().is_success()) => DataResult::success(numbers),
                    _ => DataResult::error("Some elements are not bytes"),
                }
            }),
        }
    }

    pub fn as_string_or(&self, default: &str) -> String {
        self.as_string().value.filter(|_| true).unwrap_or_else(|| default.to_string())
    }

    pub fn as_int(&self, default: i32) -> i32 {
        self.as_number().value.map_or(default, Number::int_value)
    }

    pub fn as_long(&self, default: i64) -> i64 {
        self.as_number().value.map_or(default, Number::long_value)
    }

    pub fn as_byte(&self, default: i8) -> i8 {
        self.as_number().value.map_or(default, Number::byte_value)
    }

    pub fn as_short(&self, default: i16) -> i16 {
        self.as_number().value.map_or(default, Number::short_value)
    }

    pub fn as_float(&self, default: f32) -> f32 {
        self.as_number().value.map_or(default, Number::float_value)
    }

    pub fn as_double(&self, default: f64) -> f64 {
        self.as_number().value.map_or(default, Number::double_value)
    }

    pub fn as_boolean_or(&self, default: bool) -> bool {
        self.as_boolean().value.unwrap_or(default)
    }

    pub fn as_number_or(&self, default: Number) -> Number {
        self.as_number().value.unwrap_or(default)
    }

    pub fn as_stream(&self) -> Vec<Dynamic> {
        self.as_stream_opt().value.unwrap_or_default()
    }

    pub fn as_list<T>(&self, f: impl FnMut(Dynamic) -> T) -> Vec<T> {
        self.as_stream().into_iter().map(f).collect()
    }

    pub fn as_map<K, V>(&self, mut key_mapper: impl FnMut(Dynamic) -> K, mut value_mapper: impl FnMut(Dynamic) -> V) -> Vec<(K, V)> {
        match self.get_map_values().value {
            Some(entries) => entries.into_iter().map(|(key, value)| (key_mapper(key), value_mapper(value))).collect(),
            None => Vec::new(),
        }
    }

    pub fn as_long_stream(&self) -> Vec<i64> {
        self.as_long_stream_opt().value.unwrap_or_default()
    }

    pub fn as_int_stream(&self) -> Vec<i32> {
        self.as_int_stream_opt().value.unwrap_or_default()
    }

    pub fn merge(self, value: Dynamic) -> OptionalDynamic {
        match (self.value, value.value) {
            (Some(Nbt::List(mut list)), Some(value)) => {
                list.push(value);
                OptionalDynamic(DataResult::success(Dynamic::new(Nbt::List(list))))
            }
            (None, Some(value)) => OptionalDynamic(DataResult::success(Dynamic::new(Nbt::List(vec![value])))),
            (list, _) => OptionalDynamic(DataResult { value: Some(Dynamic { value: list.clone() }), error: Some(format!("mergeToList called with not a list: {}", snbt(&list))) }),
        }
    }
}

pub struct OptionalDynamic(pub DataResult<Dynamic>);

impl OptionalDynamic {
    pub fn result(self) -> Option<Dynamic> {
        if self.0.is_success() { self.0.value } else { None }
    }

    pub fn get(self, key: &str) -> OptionalDynamic {
        match self.result() {
            Some(dynamic) => dynamic.get(key),
            None => OptionalDynamic(DataResult::error("missing")),
        }
    }

    pub fn as_string(self) -> DataResult<String> {
        match self.result() {
            Some(dynamic) => dynamic.as_string(),
            None => DataResult::error("missing"),
        }
    }

    pub fn as_number(self) -> DataResult<Number> {
        match self.result() {
            Some(dynamic) => dynamic.as_number(),
            None => DataResult::error("missing"),
        }
    }

    pub fn as_boolean(self) -> DataResult<bool> {
        self.as_number().map(|value| value.double_value() != 0.0)
    }

    pub fn as_stream_opt(self) -> DataResult<Vec<Dynamic>> {
        match self.result() {
            Some(dynamic) => dynamic.as_stream_opt(),
            None => DataResult::error("missing"),
        }
    }

    pub fn as_map_opt(self) -> DataResult<Vec<(Dynamic, Dynamic)>> {
        match self.result() {
            Some(dynamic) => dynamic.as_map_opt(),
            None => DataResult::error("missing"),
        }
    }

    pub fn as_string_or(self, default: &str) -> String {
        self.as_string().value.unwrap_or_else(|| default.to_string())
    }

    pub fn as_int(self, default: i32) -> i32 {
        self.as_number().value.map_or(default, Number::int_value)
    }

    pub fn as_long(self, default: i64) -> i64 {
        self.as_number().value.map_or(default, Number::long_value)
    }

    pub fn as_byte(self, default: i8) -> i8 {
        self.as_number().value.map_or(default, Number::byte_value)
    }

    pub fn as_short(self, default: i16) -> i16 {
        self.as_number().value.map_or(default, Number::short_value)
    }

    pub fn as_float(self, default: f32) -> f32 {
        self.as_number().value.map_or(default, Number::float_value)
    }

    pub fn as_double(self, default: f64) -> f64 {
        self.as_number().value.map_or(default, Number::double_value)
    }

    pub fn as_boolean_or(self, default: bool) -> bool {
        self.as_boolean().value.unwrap_or(default)
    }

    pub fn as_stream(self) -> Vec<Dynamic> {
        self.as_stream_opt().value.unwrap_or_default()
    }

    pub fn as_list<T>(self, f: impl FnMut(Dynamic) -> T) -> Vec<T> {
        self.as_stream().into_iter().map(f).collect()
    }

    pub fn as_map<K, V>(self, key_mapper: impl FnMut(Dynamic) -> K, value_mapper: impl FnMut(Dynamic) -> V) -> Vec<(K, V)> {
        match self.result() {
            Some(dynamic) => dynamic.as_map(key_mapper, value_mapper),
            None => Vec::new(),
        }
    }

    pub fn as_long_stream(self) -> Vec<i64> {
        match self.result() {
            Some(dynamic) => dynamic.as_long_stream(),
            None => Vec::new(),
        }
    }

    pub fn as_int_stream(self) -> Vec<i32> {
        match self.result() {
            Some(dynamic) => dynamic.as_int_stream(),
            None => Vec::new(),
        }
    }

    pub fn or_else_empty_map(self) -> Dynamic {
        self.result().unwrap_or_else(|| Dynamic::new(Nbt::Compound(TagMap::new())))
    }

    pub fn or_else_empty_list(self) -> Dynamic {
        self.result().unwrap_or_else(|| Dynamic::new(Nbt::List(Vec::new())))
    }

    pub fn is_present(&self) -> bool {
        self.0.is_success()
    }
}
