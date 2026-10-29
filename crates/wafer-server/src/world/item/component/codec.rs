use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Reader, Writer};

use crate::commands::arguments::parse_identifier;
use crate::nbt;
use crate::util::codec::DecodeResult;
use crate::util::hash_ops::{self, HashCode};

pub trait Custom: Send + Sync {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String>;
    fn hash(&self, value: &Nbt) -> HashCode;
    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String>;
    fn read(&self, r: &mut Reader) -> Result<Nbt, String>;
}

#[derive(Clone, Copy)]
pub enum RangeError {
    Within,
    NonNegative,
    Positive,
    Outside,
}

#[derive(Clone)]
pub enum Kind {
    Required,
    Optional,
    Default(Nbt),
    Lenient(Nbt),
    LenientOptional,
    AlwaysPresent(Nbt),
    Plain,
}

pub struct Field {
    pub name: &'static str,
    pub codec: Codec,
    pub stream: Stream,
    pub kind: Kind,
}

pub enum Selector {
    Registry(&'static str),
    Enum,
}

pub struct Dispatch {
    pub key: &'static str,
    pub selector: Selector,
    pub cases: Vec<(&'static str, Vec<Field>)>,
}

pub enum Codec {
    Unit,
    Bool,
    Byte,
    UnsignedByte,
    Short,
    Int(i32, i32, RangeError),
    Long(i64, i64, RangeError),
    Float(f32, f32, bool, RangeError),
    Double,
    DoubleRange(f64, f64),
    Func(fn(&Nbt) -> DecodeResult<Nbt>),
    String(usize, usize),
    Identifier,
    Element(&'static str),
    Holder(&'static str, Option<Box<Codec>>),
    HolderSet(&'static str),
    Enum(&'static [&'static str]),
    IdEnum(&'static [(&'static str, i32)]),
    Lazy(fn() -> &'static (Codec, Stream)),
    Normalize(Box<Codec>, fn(Nbt) -> Result<Nbt, String>),
    List(Box<Codec>, usize, usize),
    SizedList(Box<Codec>, usize, usize),
    Compact(Box<Codec>),
    Map(Box<Codec>, Box<Codec>),
    Record(Vec<Field>),
    Dispatch(Box<Dispatch>),
    Either(Box<Codec>, Box<Codec>),
    Xor(Box<Codec>, Box<Codec>),
    Alt(Box<Codec>, Box<Codec>, fn(&Nbt) -> Result<Nbt, String>),
    Compound,
    AnyTag,
    Uuid,
    Custom(Box<dyn Custom>),
}

pub enum Stream {
    Unit,
    Bool,
    Byte,
    Short,
    VarInt,
    Int,
    VarLong,
    Long,
    Float,
    Double,
    Utf8(usize),
    Identifier,
    Registry(&'static str),
    Holder(&'static str, Option<Box<Stream>>),
    HolderSet(&'static str),
    Enum,
    List(Box<Stream>, usize),
    FixedList(Box<Stream>, usize),
    Map(Box<Stream>, Box<Stream>, usize),
    Record,
    Dispatch,
    Either(Box<Stream>, Box<Stream>),
    Nbt,
    Uuid,
    Custom,
    Lazy,
}

impl Field {
    pub fn new(name: &'static str, codec: Codec, stream: Stream, kind: Kind) -> Field {
        Field { name, codec, stream, kind }
    }
}

fn float_text(v: f32) -> String {
    nbt::java_float_to_string(v)
}

fn range_error<T: std::fmt::Display>(kind: RangeError, value: String, min: T, max: T) -> String {
    match kind {
        RangeError::Within => format!("Value must be within range [{min};{max}]: {value}"),
        RangeError::NonNegative => format!("Value must be non-negative: {value}"),
        RangeError::Positive => format!("Value must be positive: {value}"),
        RangeError::Outside => format!("Value {value} outside of range [{min}:{max}]"),
    }
}

fn number(tag: &Nbt) -> Result<&Nbt, String> {
    match tag {
        Nbt::Byte(_) if nbt::is_json_source() => Err(format!("Not a number: {}", nbt::to_string(tag))),
        Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) => Ok(tag),
        _ if nbt::is_json_source() => Err(format!("Not a number: {}", nbt::to_string(tag))),
        _ => Err("Not a number".into()),
    }
}

fn big_decimal_long_value(v: f64) -> i64 {
    if v.abs() < 9.2e18 {
        return v as i64;
    }
    let bits = v.to_bits();
    let shift = ((bits >> 52) & 0x7ff) as i64 - 1075;
    let mantissa = (bits & ((1u64 << 52) - 1)) | (1u64 << 52);
    let magnitude = if shift >= 64 { 0 } else { (mantissa << shift) as i64 };
    if v < 0.0 { magnitude.wrapping_neg() } else { magnitude }
}

pub fn as_i64(tag: &Nbt) -> i64 {
    match *tag {
        Nbt::Byte(v) => v as i64,
        Nbt::Short(v) => v as i64,
        Nbt::Int(v) => v as i64,
        Nbt::Long(v) => v,
        Nbt::Float(v) => v as i64,
        Nbt::Double(v) if nbt::is_json_source() => big_decimal_long_value(v),
        Nbt::Double(v) => v as i64,
        _ => 0,
    }
}

pub fn as_i32(tag: &Nbt) -> i32 {
    match *tag {
        Nbt::Float(v) => v as i32,
        Nbt::Double(v) if nbt::is_json_source() => big_decimal_long_value(v) as i32,
        Nbt::Double(v) => v as i32,
        _ => as_i64(tag) as i32,
    }
}

pub fn as_f64(tag: &Nbt) -> f64 {
    match *tag {
        Nbt::Float(v) => v as f64,
        Nbt::Double(v) => v,
        _ => as_i64(tag) as f64,
    }
}

pub fn as_f32(tag: &Nbt) -> f32 {
    match *tag {
        Nbt::Float(v) => v,
        _ => as_f64(tag) as f32,
    }
}

pub fn string(tag: &Nbt) -> Result<&str, String> {
    match tag {
        Nbt::String(s) => Ok(s),
        other if nbt::is_json_source() => Err(format!("Not a string: {}", nbt::to_string(other))),
        _ => Err("Not a string".into()),
    }
}

pub fn map(tag: &Nbt) -> Result<&[(String, Nbt)], String> {
    match tag {
        Nbt::Compound(fields) => Ok(fields),
        other if nbt::is_json_source() => Err(format!("Not a JSON object: {}", nbt::to_string(other))),
        other => Err(format!("Not a map: {}", nbt::to_string(other))),
    }
}

pub fn list(tag: &Nbt) -> Result<Vec<Nbt>, String> {
    match tag {
        Nbt::List(items) => Ok(items.clone()),
        Nbt::ByteArray(v) => Ok(v.iter().map(|&b| Nbt::Byte(b)).collect()),
        Nbt::IntArray(v) => Ok(v.iter().map(|&b| Nbt::Int(b)).collect()),
        Nbt::LongArray(v) => Ok(v.iter().map(|&b| Nbt::Long(b)).collect()),
        other if nbt::is_json_source() => Err(format!("Not a json array: {}", nbt::to_string(other))),
        other => Err(format!("Not a list: {}", nbt::to_string(other))),
    }
}

pub fn identifier(tag: &Nbt) -> Result<String, String> {
    crate::resources::identifier::read(string(tag)?)
}

pub fn registry(name: &str) -> &'static [&'static str] {
    wafer_data::registry_entries(name).unwrap_or(&[])
}

pub fn registry_id(name: &str, entry: &str) -> Result<i32, String> {
    registry(name).iter().position(|e| *e == entry).map(|i| i as i32).ok_or_else(|| format!("Can't find id for '{entry}' in registry {name}"))
}

pub fn network_registry_id(name: &str, entry: &str) -> Result<i32, String> {
    registry_id(name, entry).map(|id| crate::core::data_registries::network_id(name, id))
}

fn registry_entry(name: &str, id: i32) -> Result<&'static str, String> {
    registry(name).get(crate::core::data_registries::from_network_id(name, id) as usize).copied().ok_or_else(|| format!("No value with id {id} in registry {name}"))
}

pub fn tag_exists(registry: &str, tag: &str) -> bool {
    wafer_data::tags().iter().any(|r| r.registry == registry && r.tags.iter().any(|t| t.0 == tag))
}

fn missing_key(name: &str, input: &Nbt) -> String {
    format!("No key {name} in MapLike[{}]", nbt::to_string(input))
}

fn join(errors: Vec<String>) -> Result<(), String> {
    if errors.is_empty() { Ok(()) } else { Err(errors.join("; ")) }
}

impl Codec {
    pub fn list(elem: Codec) -> Codec {
        Codec::List(Box::new(elem), 0, i32::MAX as usize)
    }

    pub fn list_max(elem: Codec, max: usize) -> Codec {
        Codec::List(Box::new(elem), 0, max)
    }

    pub fn int() -> Codec {
        Codec::Int(i32::MIN, i32::MAX, RangeError::Within)
    }

    pub fn float() -> Codec {
        Codec::Float(f32::NEG_INFINITY, f32::INFINITY, false, RangeError::Within)
    }

    pub fn string() -> Codec {
        Codec::String(0, i32::MAX as usize)
    }

    pub fn long() -> Codec {
        Codec::Long(i64::MIN, i64::MAX, RangeError::Within)
    }

    pub fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        match self {
            Codec::Unit => map(tag).map(|_| Nbt::Compound(Vec::new())),
            Codec::Bool if nbt::is_json_source() => match tag {
                Nbt::Byte(value) => Ok(Nbt::Byte((*value != 0) as i8)),
                _ => Err(format!("Not a boolean: {}", nbt::to_string(tag))),
            },
            Codec::Bool => number(tag).map(|n| Nbt::Byte((as_f64(n) != 0.0) as i8)),
            Codec::Byte => number(tag).map(|n| Nbt::Byte(as_i64(n) as i8)),
            Codec::UnsignedByte => number(tag).map(|n| Nbt::Byte(as_i64(n) as i8)),
            Codec::Short => number(tag).map(|n| Nbt::Short(as_i64(n) as i16)),
            Codec::Int(min, max, err) => {
                let v = as_i32(number(tag)?);
                if (*min..=*max).contains(&v) { Ok(Nbt::Int(v)) } else { Err(range_error(*err, v.to_string(), min, max)) }
            }
            Codec::Long(min, max, err) => {
                let v = as_i64(number(tag)?);
                if (*min..=*max).contains(&v) { Ok(Nbt::Long(v)) } else { Err(range_error(*err, v.to_string(), min, max)) }
            }
            Codec::Float(min, max, exclusive, err) => {
                let v = as_f32(number(tag)?);
                let unbounded = *min == f32::NEG_INFINITY && *max == f32::INFINITY;
                let above_min = if *exclusive { v.total_cmp(min).is_gt() } else { v.total_cmp(min).is_ge() };
                let ok = unbounded || (above_min && v.total_cmp(max).is_le());
                if ok { Ok(Nbt::Float(v)) } else { Err(range_error(*err, float_text(v), float_text(*min), float_text(*max))) }
            }
            Codec::Double => number(tag).map(|n| Nbt::Double(as_f64(n))),
            Codec::DoubleRange(min, max) => {
                let v = as_f64(number(tag)?);
                if v.total_cmp(min).is_ge() && v.total_cmp(max).is_le() {
                    Ok(Nbt::Double(v))
                } else {
                    Err(format!("Value {} outside of range [{}:{}]", nbt::java_double_to_string(v), nbt::java_double_to_string(*min), nbt::java_double_to_string(*max)))
                }
            }
            Codec::String(min, max) => {
                let s = string(tag)?;
                let len = s.encode_utf16().count();
                if len < *min {
                    Err(format!("String \"{s}\" is too short: {len} < {min}"))
                } else if len > *max {
                    Err(format!("String \"{s}\" is too long: {len} > {max}"))
                } else {
                    Ok(Nbt::String(s.into()))
                }
            }
            Codec::Identifier => identifier(tag).map(Nbt::String),
            Codec::Element(reg) => {
                let id = identifier(tag)?;
                if registry(reg).contains(&id.as_str()) {
                    Ok(Nbt::String(id))
                } else {
                    Err(format!("Unknown registry key in ResourceKey[minecraft:root / {reg}]: {id}"))
                }
            }
            Codec::Holder(_, Some(direct)) if matches!(tag, Nbt::String(_)) && identifier(tag).is_err() => direct.decode(tag),
            Codec::Holder(reg, direct) => match (tag, direct) {
                (Nbt::String(_), _) if direct.is_none() || identifier(tag).is_ok() => {
                    let id = identifier(tag)?;
                    if registry(reg).contains(&id.as_str()) {
                        crate::core::data_registries::known_reference(reg, &id);
                        Ok(Nbt::String(id))
                    } else if !crate::util::codec::is_nbt_ops() && crate::core::data_registries::is_loading() && crate::core::data_registries::get(reg).is_some() {
                        crate::core::data_registries::unknown_reference(reg, &id);
                        Ok(Nbt::String(id))
                    } else {
                        Err(format!("Failed to get element {id} from registry {reg}"))
                    }
                }
                (_, Some(direct)) => direct.decode(tag),
                (_, None) => Err("Not a string".into()),
            },
            Codec::HolderSet(reg) => decode_holder_set(reg, tag),
            Codec::Enum(names) => {
                let s = string(tag)?;
                if names.contains(&s) { Ok(Nbt::String(s.into())) } else { Err(format!("Unknown element name:{s}")) }
            }
            Codec::IdEnum(names) => {
                let s = string(tag)?;
                if names.iter().any(|(n, _)| *n == s) { Ok(Nbt::String(s.into())) } else { Err(format!("Unknown element name:{s}")) }
            }
            Codec::Func(_)
            | Codec::Lazy(_)
            | Codec::Normalize(..)
            | Codec::List(..)
            | Codec::SizedList(..)
            | Codec::Compact(_)
            | Codec::Map(..)
            | Codec::Record(_)
            | Codec::Dispatch(_)
            | Codec::Either(..)
            | Codec::Xor(..)
            | Codec::Alt(..) => self.decode_partial(tag).into_result(),
            Codec::Compound => map(tag).map(|_| tag.clone()),
            Codec::AnyTag => Ok(tag.clone()),
            Codec::Uuid => decode_uuid(tag),
            Codec::Custom(c) => c.decode(tag),
        }
    }

    pub fn decode_partial(&self, tag: &Nbt) -> DecodeResult<Nbt> {
        match self {
            Codec::Func(f) => f(tag),
            Codec::HolderSet(reg) => decode_holder_set_partial(reg, tag),
            Codec::Lazy(pair) => pair().0.decode_partial(tag),
            Codec::Holder(_, Some(direct)) if !matches!(tag, Nbt::String(_)) || identifier(tag).is_err() => direct.decode_partial(tag),
            Codec::Normalize(inner, f) => inner.decode_partial(tag).flat_map(|value| DecodeResult::from(f(value))),
            Codec::List(elem, min, max) => {
                let items = match list(tag) {
                    Ok(items) => items,
                    Err(error) => return DecodeResult::error(error),
                };
                let (out, mut errors) = decode_elements(elem, &items);
                if *min > 1 && out.len() < *min {
                    return DecodeResult::error(format!("List is too short: {}, expected range [{min}-{max}]", out.len()));
                }
                if out.len() > *max {
                    return DecodeResult::error(format!("List is too long: {} > {max}", out.len()));
                }
                errors.reverse();
                if out.len() < *min {
                    errors.push("List must have contents".into());
                    return DecodeResult { value: None, error: Some(errors.join("; ")) };
                }
                DecodeResult { value: Some(Nbt::List(out)), error: (!errors.is_empty()).then(|| errors.join("; ")) }
            }
            Codec::SizedList(elem, min, max) => {
                let items = match list(tag) {
                    Ok(items) => items,
                    Err(error) => return DecodeResult::error(error),
                };
                let (out, mut errors) = decode_elements_up_to(elem, &items, *max);
                if out.len() < *min {
                    return DecodeResult::error(format!("List is too short: {}, expected range [{min}-{max}]", out.len()));
                }
                if items.len() > *max {
                    return DecodeResult { value: Some(Nbt::List(out)), error: Some(format!("List is too long: {}, expected range [{min}-{max}]", items.len())) };
                }
                errors.reverse();
                DecodeResult { value: Some(Nbt::List(out)), error: (!errors.is_empty()).then(|| errors.join("; ")) }
            }
            Codec::Compact(elem) => {
                let first = match list(tag) {
                    Ok(items) => {
                        let (out, mut errors) = decode_elements(elem, &items);
                        errors.reverse();
                        DecodeResult { value: Some(out), error: (!errors.is_empty()).then(|| errors.join("; ")) }
                    }
                    Err(error) => DecodeResult::error(error),
                };
                crate::util::codec::either_result(first, || elem.decode_partial(tag)).map(|either| match either {
                    Ok(mut items) if items.len() == 1 => items.pop().unwrap(),
                    Ok(items) => Nbt::List(items),
                    Err(single) => single,
                })
            }
            Codec::Map(key, value) => {
                let fields = match map(tag) {
                    Ok(fields) => fields,
                    Err(error) => return DecodeResult::error(error),
                };
                let mut out = Vec::with_capacity(fields.len());
                let mut errors = Vec::new();
                let mut failed = Vec::new();
                for i in crate::util::codec::map_entry_order(fields) {
                    let (k, v) = &fields[i];
                    let key_result = key.decode_partial(&Nbt::String(k.clone()));
                    let value_result = value.decode_partial(v);
                    let entry_error = match (&value_result.error, &key_result.error) {
                        (Some(value_error), Some(key_error)) => Some(format!("{value_error}; {key_error}")),
                        (Some(error), None) | (None, Some(error)) => Some(error.clone()),
                        (None, None) => None,
                    };
                    if let (Some(Nbt::String(key)), Some(value)) = (key_result.value, value_result.value) {
                        out.push((key, value));
                    }
                    if let Some(error) = entry_error {
                        errors.push(error);
                        failed.push((k.clone(), v.clone()));
                    }
                }
                let error = (!errors.is_empty()).then(|| crate::util::codec::map_entry_failures(errors, failed));
                DecodeResult { value: Some(Nbt::Compound(out)), error }
            }
            Codec::Record(fields) => {
                let input = match map(tag) {
                    Ok(input) => input,
                    Err(error) => return DecodeResult::error(error),
                };
                let mut out = Vec::new();
                let mut errors = Vec::new();
                let complete = decode_fields(fields, tag, input, &mut out, &mut errors);
                finish_record(complete.then(|| Nbt::Compound(out)), errors)
            }
            Codec::Dispatch(d) => decode_dispatch(d, tag),
            Codec::Either(a, b) => crate::util::codec::either_result(a.decode_partial(tag), || b.decode_partial(tag)).map(|either| either.unwrap_or_else(|value| value)),
            Codec::Xor(a, b) => {
                let first = a.decode_partial(tag);
                let second = b.decode_partial(tag);
                match (first.is_success(), second.is_success()) {
                    (true, true) => {
                        let message = format!(
                            "Both alternatives read successfully, can not pick the correct one; first: (Left[{}], {}) second: (Right[{}], {})",
                            nbt::to_string(first.value.as_ref().unwrap_or(&Nbt::End)),
                            nbt::to_string(tag),
                            nbt::to_string(second.value.as_ref().unwrap_or(&Nbt::End)),
                            nbt::to_string(tag)
                        );
                        DecodeResult { value: first.value, error: Some(message) }
                    }
                    (true, false) => first,
                    (false, true) => second,
                    (false, false) => DecodeResult {
                        value: first.value.and(second.value),
                        error: Some(format!("{}; {}", second.error.unwrap_or_default(), first.error.unwrap_or_default())),
                    },
                }
            }
            Codec::Alt(a, b, convert) => crate::util::codec::either_result(a.decode_partial(tag), || b.decode_partial(tag).flat_map(|value| DecodeResult::from(convert(&value))))
                .map(|either| either.unwrap_or_else(|value| value)),
            _ => DecodeResult::from(self.decode(tag)),
        }
    }

    pub fn hash(&self, value: &Nbt) -> HashCode {
        match self {
            Codec::Unit => hash_ops::create_map(Vec::new()),
            Codec::Bool => hash_ops::create_boolean(as_i64(value) != 0),
            Codec::Holder(_, Some(direct)) if !matches!(value, Nbt::String(_)) => direct.hash(value),
            Codec::List(elem, _, _) | Codec::SizedList(elem, _, _) => hash_ops::create_list(value.as_list().iter().map(|v| elem.hash(v))),
            Codec::Compact(elem) => match value {
                Nbt::List(items) if !elem.accepts(value) => hash_ops::create_list(items.iter().map(|v| elem.hash(v))),
                single => elem.hash(single),
            },
            Codec::Map(key, v) => match value {
                Nbt::Compound(fields) => hash_ops::create_map(fields.iter().map(|(k, x)| (key.hash(&Nbt::String(k.clone())), v.hash(x))).collect()),
                other => hash_ops::hash_nbt(other),
            },
            Codec::Record(fields) => hash_record(fields, value, None),
            Codec::Dispatch(d) => {
                let case = value.get(d.key).and_then(Nbt::as_str).and_then(|t| d.cases.iter().find(|(n, _)| *n == t));
                match case {
                    Some((_, fields)) => hash_record(fields, value, Some(d.key)),
                    None => hash_ops::hash_nbt(value),
                }
            }
            Codec::Either(a, b) | Codec::Xor(a, b) | Codec::Alt(a, b, _) => {
                if matches!(self, Codec::Alt(..)) || a.accepts(value) {
                    a.hash(value)
                } else {
                    b.hash(value)
                }
            }
            Codec::Uuid => match value {
                Nbt::IntArray(v) => hash_ops::create_int_list(v),
                other => hash_ops::hash_nbt(other),
            },
            Codec::Custom(c) => c.hash(value),
            Codec::Lazy(pair) => pair().0.hash(value),
            Codec::Normalize(inner, _) => inner.hash(value),
            _ => hash_ops::hash_nbt(value),
        }
    }

    pub fn accepts(&self, value: &Nbt) -> bool {
        self.decode(value).is_ok()
    }

    fn enum_id(&self, name: &str) -> Option<i32> {
        match self {
            Codec::Enum(names) => names.iter().position(|n| *n == name).map(|i| i as i32),
            Codec::IdEnum(names) => names.iter().find(|(n, _)| *n == name).map(|(_, id)| *id),
            _ => None,
        }
    }

    fn enum_name(&self, id: i32) -> &'static str {
        match self {
            Codec::Enum(names) => names.get(id as usize).or(names.first()).copied().unwrap_or(""),
            Codec::IdEnum(names) => names.iter().find(|(_, i)| *i == id).or(names.first()).map_or("", |(n, _)| n),
            _ => "",
        }
    }
}

fn decode_elements(elem: &Codec, items: &[Nbt]) -> (Vec<Nbt>, Vec<String>) {
    decode_elements_up_to(elem, items, usize::MAX)
}

fn decode_elements_up_to(elem: &Codec, items: &[Nbt], max: usize) -> (Vec<Nbt>, Vec<String>) {
    let mut out = Vec::with_capacity(items.len().min(max));
    let mut errors = Vec::new();
    for item in items {
        if out.len() >= max {
            continue;
        }
        let result = elem.decode_partial(item);
        if let Some(error) = result.error {
            errors.push(error);
        }
        if let Some(value) = result.value {
            out.push(value);
        }
    }
    (out, errors)
}

fn finish_record(value: Option<Nbt>, mut errors: Vec<String>) -> DecodeResult<Nbt> {
    errors.reverse();
    DecodeResult { value, error: (!errors.is_empty()).then(|| errors.join("; ")) }
}

fn holder_set_element(reg: &'static str, item: &Nbt) -> crate::util::codec::DecodeResult<Nbt> {
    let id = match identifier(item) {
        Ok(id) => id,
        Err(error) => return crate::util::codec::DecodeResult::error(error),
    };
    if registry(reg).contains(&id.as_str()) {
        crate::core::data_registries::known_reference(reg, &id);
        crate::util::codec::DecodeResult::success(Nbt::String(id))
    } else if !crate::util::codec::is_nbt_ops() && crate::core::data_registries::is_loading() && crate::core::data_registries::get(reg).is_some() {
        crate::core::data_registries::unknown_reference(reg, &id);
        crate::util::codec::DecodeResult::success(Nbt::String(id))
    } else {
        crate::util::codec::DecodeResult::error(format!("Failed to get element {id} from registry {reg}"))
    }
}

pub fn inline_holder_set(registry: &'static str, tag: &Nbt, holder: &Codec, always_use_list: bool) -> DecodeResult<Nbt> {
    decode_holder_set_with(registry, tag, |element| holder.decode_partial(element), always_use_list)
}

fn decode_holder_set(reg: &'static str, tag: &Nbt) -> Result<Nbt, String> {
    decode_holder_set_partial(reg, tag).into_result()
}

fn decode_holder_set_partial(reg: &'static str, tag: &Nbt) -> DecodeResult<Nbt> {
    decode_holder_set_with(reg, tag, |item| holder_set_element(reg, item), false)
}

fn decode_holder_set_with(reg: &'static str, tag: &Nbt, element: impl Fn(&Nbt) -> DecodeResult<Nbt>, always_use_list: bool) -> DecodeResult<Nbt> {
    if let Nbt::String(s) = tag
        && let Some(location) = s.strip_prefix('#')
    {
        return match identifier(&Nbt::String(location.into())) {
            Ok(id) => match tag_exists(reg, &id).then_some(()).map_or_else(|| crate::core::data_registries::missing_tag(reg, &id), |_| None) {
                None => DecodeResult::success(Nbt::String(format!("#{id}"))),
                Some(error) => DecodeResult::error(error),
            },
            Err(error) => {
                let second = if always_use_list { crate::util::codec::list_partial(tag, &element) } else { crate::util::codec::compact_list_partial(tag, &element) };
                DecodeResult::error(format!("Failed to parse either. First: {error}; Second: {}", second.error.unwrap_or_default()))
            }
        };
    }
    let result = crate::util::codec::holder_set_partial_with(tag, reg, &element, always_use_list);
    result.map(|mut out| if out.len() == 1 { out.pop().unwrap() } else { Nbt::List(out) })
}

fn decode_uuid(tag: &Nbt) -> Result<Nbt, String> {
    match tag {
        Nbt::IntArray(v) if v.len() == 4 => Ok(tag.clone()),
        Nbt::IntArray(v) => Err(format!("Input is not a list of 4 ints: {}", v.len())),
        _ => {
            let items = list(tag)?;
            if items.len() != 4 {
                return Err(format!("Input is not a list of 4 ints: {}", items.len()));
            }
            let mut out = Vec::new();
            for item in &items {
                out.push(as_i32(number(item)?));
            }
            Ok(Nbt::IntArray(out))
        }
    }
}

fn decode_fields(fields: &[Field], tag: &Nbt, input: &[(String, Nbt)], out: &mut Vec<(String, Nbt)>, errors: &mut Vec<String>) -> bool {
    let mut complete = true;
    for field in fields {
        let present = input.iter().find(|(k, v)| k == field.name && !matches!(v, Nbt::End)).map(|(_, v)| v);
        match (&field.kind, present) {
            (Kind::Required, None) => {
                errors.push(missing_key(field.name, tag));
                complete = false;
            }
            (Kind::AlwaysPresent(default), None) => out.push((field.name.into(), default.clone())),
            (_, None) => {}
            (kind, Some(v)) => {
                let result = field.codec.decode_partial(v);
                let failed = result.error.is_some();
                let lenient = matches!(kind, Kind::Lenient(_) | Kind::LenientOptional);
                if let Some(error) = result.error
                    && !lenient
                {
                    errors.push(error);
                }
                match result.value {
                    _ if lenient && failed => {}
                    Some(v) => match kind {
                        Kind::Default(d) | Kind::Lenient(d) if *d == v => {}
                        _ => out.push((field.name.into(), v)),
                    },
                    None => {
                        if matches!(kind, Kind::Required | Kind::Plain) {
                            complete = false;
                        }
                    }
                }
            }
        }
    }
    complete
}

fn decode_dispatch(d: &Dispatch, tag: &Nbt) -> DecodeResult<Nbt> {
    let input = match map(tag) {
        Ok(input) => input,
        Err(error) => return DecodeResult::error(error),
    };
    let Some((_, ty)) = input.iter().find(|(k, v)| k == d.key && !matches!(v, Nbt::End)) else { return DecodeResult::error(missing_key(d.key, tag)) };
    let name = match d.selector {
        Selector::Registry(reg) => {
            let id = match identifier(ty) {
                Ok(id) => id,
                Err(error) => return DecodeResult::error(error),
            };
            if !d.cases.iter().any(|(n, _)| *n == id) {
                return DecodeResult::error(format!("Unknown registry key in ResourceKey[minecraft:root / {reg}]: {id}"));
            }
            id
        }
        Selector::Enum => {
            let s = match string(ty) {
                Ok(s) => s,
                Err(error) => return DecodeResult::error(error),
            };
            if !d.cases.iter().any(|(n, _)| *n == s) {
                return DecodeResult::error(format!("Unknown element name:{s}"));
            }
            s.to_string()
        }
    };
    let (_, fields) = d.cases.iter().find(|(n, _)| *n == name).unwrap();
    let mut out = Vec::new();
    let mut errors = Vec::new();
    let complete = decode_fields(fields, tag, input, &mut out, &mut errors);
    out.push((d.key.to_string(), Nbt::String(name)));
    finish_record(complete.then(|| Nbt::Compound(out)), errors)
}

fn hash_record(fields: &[Field], value: &Nbt, key: Option<&str>) -> HashCode {
    let Nbt::Compound(entries) = value else { return hash_ops::hash_nbt(value) };
    let mut out = Vec::with_capacity(entries.len());
    for (k, v) in entries {
        let h = match fields.iter().find(|f| f.name == k) {
            Some(f) => f.codec.hash(v),
            None if Some(k.as_str()) == key => hash_ops::create_string(v.as_str().unwrap_or("")),
            None => hash_ops::hash_nbt(v),
        };
        out.push((hash_ops::create_string(k), h));
    }
    hash_ops::create_map(out)
}

fn elements<'a>(codec: &Codec, value: &'a Nbt) -> Vec<&'a Nbt> {
    match (codec, value) {
        (Codec::Compact(elem), Nbt::List(items)) if !elem.accepts(value) => items.iter().collect(),
        (Codec::Compact(_), single) => vec![single],
        (_, Nbt::List(items)) => items.iter().collect(),
        _ => Vec::new(),
    }
}

fn element_codec(codec: &Codec) -> &Codec {
    match codec {
        Codec::List(elem, _, _) | Codec::SizedList(elem, _, _) | Codec::Compact(elem) => elem,
        other => other,
    }
}

fn write_count(w: &mut Writer, n: usize, max: usize) -> Result<(), String> {
    if n > max {
        return Err(format!("{n} elements exceeded max size of: {max}"));
    }
    w.varint(n as i32);
    Ok(())
}

fn read_count(r: &mut Reader, max: usize) -> Result<usize, String> {
    let n = r.varint().map_err(|e| e.to_string())?;
    if n as i64 > max as i64 {
        return Err(format!("{n} elements exceeded max size of: {max}"));
    }
    Ok(n.max(0) as usize)
}

macro_rules! rd {
    ($e:expr) => {
        $e.map_err(|e| e.to_string())?
    };
}

pub fn read_utf8(r: &mut Reader, max: usize) -> Result<String, String> {
    Ok(rd!(r.string(max)).to_string())
}

pub fn write(codec: &Codec, stream: &Stream, value: &Nbt, w: &mut Writer) -> Result<(), String> {
    match codec {
        Codec::Normalize(inner, _) => return write(inner, stream, value, w),
        Codec::Alt(primary, _, _) if !matches!(stream, Stream::Either(..)) => return write(primary, stream, value, w),
        _ => {}
    }
    match stream {
        Stream::Lazy => match codec {
            Codec::Lazy(pair) => {
                let (c, s) = pair();
                write(c, s, value, w)?
            }
            _ => return Err("not a recursive codec".into()),
        },
        Stream::Unit => {}
        Stream::Bool => w.bool(as_i64(value) != 0),
        Stream::Byte => w.u8(java_i64(codec, value) as u8),
        Stream::Short => w.bytes(&(java_i64(codec, value) as i16).to_be_bytes()),
        Stream::VarInt => w.varint(java_i64(codec, value) as i32),
        Stream::Int => w.i32(java_i64(codec, value) as i32),
        Stream::VarLong => w.varlong(java_i64(codec, value)),
        Stream::Long => w.i64(java_i64(codec, value)),
        Stream::Float => w.f32(as_f32(value)),
        Stream::Double => w.f64(as_f64(value)),
        Stream::Utf8(max) => {
            let s = value.as_str().unwrap_or("");
            let length = s.encode_utf16().count();
            if length > *max {
                return Err(format!("String too big (was {length} characters, max {max})"));
            }
            w.string(s)
        }
        Stream::Identifier => w.string(value.as_str().unwrap_or("")),
        Stream::Registry(reg) => w.varint(network_registry_id(reg, value.as_str().unwrap_or(""))?),
        Stream::Holder(reg, direct_stream) => match value {
            Nbt::String(id) => w.varint(network_registry_id(reg, id)? + 1),
            direct => {
                w.varint(0);
                match (codec, direct_stream) {
                    (Codec::Holder(_, Some(inner)), Some(s)) => write(inner, s, direct, w)?,
                    _ => return Err("not a direct holder".into()),
                }
            }
        },
        Stream::HolderSet(reg) => match value {
            Nbt::String(s) if s.starts_with('#') => {
                w.varint(0);
                w.string(&s[1..]);
            }
            Nbt::String(id) => {
                w.varint(2);
                w.varint(network_registry_id(reg, id)?);
            }
            other => {
                let items = other.as_list();
                w.varint(items.len() as i32 + 1);
                for item in items {
                    w.varint(network_registry_id(reg, item.as_str().unwrap_or(""))?);
                }
            }
        },
        Stream::Enum => {
            let name = value.as_str().unwrap_or("");
            w.varint(codec.enum_id(name).ok_or_else(|| format!("Unknown element {name}"))?);
        }
        Stream::List(elem_stream, max) => {
            let items = elements(codec, value);
            write_count(w, items.len(), *max)?;
            let elem = element_codec(codec);
            for item in items {
                write(elem, elem_stream, item, w)?;
            }
        }
        Stream::FixedList(elem_stream, size) => {
            let items = elements(codec, value);
            if items.len() != *size {
                return Err(format!("Invalid list size, expected {size}, but got {}", items.len()));
            }
            let elem = element_codec(codec);
            for item in items {
                write(elem, elem_stream, item, w)?;
            }
        }
        Stream::Map(ks, vs, max) => {
            let (Codec::Map(kc, vc), Nbt::Compound(fields)) = (codec, value) else { return Err("not a map".into()) };
            write_count(w, fields.len(), *max)?;
            for i in wafer_protocol::nbt::java_hash_map_order(&fields.iter().map(|(k, _)| k.as_str()).collect::<Vec<_>>()) {
                let (k, v) = &fields[i];
                write(kc, ks, &Nbt::String(k.clone()), w)?;
                write(vc, vs, v, w)?;
            }
        }
        Stream::Record => match codec {
            Codec::Record(fields) => write_fields(fields, value, w)?,
            _ => return Err("not a record".into()),
        },
        Stream::Dispatch => {
            let Codec::Dispatch(d) = codec else { return Err("not a dispatch".into()) };
            let name = value.get(d.key).and_then(Nbt::as_str).unwrap_or("");
            let index = d.cases.iter().position(|(n, _)| *n == name).ok_or_else(|| format!("Unknown type {name}"))?;
            match d.selector {
                Selector::Registry(reg) => w.varint(network_registry_id(reg, name)?),
                Selector::Enum => w.varint(index as i32),
            }
            write_fields(&d.cases[index].1, value, w)?;
        }
        Stream::Either(left, right) => {
            let (Codec::Either(a, b) | Codec::Alt(a, b, _)) = codec else { return Err("not an either".into()) };
            if a.accepts(value) {
                w.bool(true);
                write(a, left, value, w)?;
            } else {
                w.bool(false);
                write(b, right, value, w)?;
            }
        }
        Stream::Nbt => value.write(w),
        Stream::Uuid => {
            let v = match value {
                Nbt::IntArray(v) if v.len() == 4 => v.clone(),
                _ => vec![0; 4],
            };
            w.i64(((v[0] as u32 as i64) << 32) | v[1] as u32 as i64);
            w.i64(((v[2] as u32 as i64) << 32) | v[3] as u32 as i64);
        }
        Stream::Custom => match codec {
            Codec::Custom(c) => c.write(value, w)?,
            _ => return Err("no custom codec".into()),
        },
    }
    Ok(())
}

fn write_fields(fields: &[Field], value: &Nbt, w: &mut Writer) -> Result<(), String> {
    for field in fields {
        let present = value.get(field.name);
        match (&field.kind, present) {
            (Kind::Optional | Kind::LenientOptional, None) => w.bool(false),
            (Kind::Optional | Kind::LenientOptional, Some(v)) => {
                w.bool(true);
                write(&field.codec, &field.stream, v, w)?;
            }
            (Kind::Default(d) | Kind::Lenient(d) | Kind::AlwaysPresent(d), v) => write(&field.codec, &field.stream, v.unwrap_or(d), w)?,
            (Kind::Required | Kind::Plain, Some(v)) => write(&field.codec, &field.stream, v, w)?,
            (Kind::Required | Kind::Plain, None) => return Err(format!("missing field {}", field.name)),
        }
    }
    Ok(())
}

fn java_i64(codec: &Codec, value: &Nbt) -> i64 {
    match (codec, value) {
        (Codec::UnsignedByte, Nbt::Byte(b)) => *b as u8 as i64,
        (Codec::Bool, v) => (as_i64(v) != 0) as i64,
        (_, v) => as_i64(v),
    }
}

fn canonical_number(codec: &Codec, v: i64) -> Nbt {
    match codec {
        Codec::Byte | Codec::UnsignedByte => Nbt::Byte(v as i8),
        Codec::Short => Nbt::Short(v as i16),
        Codec::Long(..) => Nbt::Long(v),
        Codec::Bool => Nbt::Byte((v != 0) as i8),
        Codec::Float(..) => Nbt::Float(v as f32),
        Codec::Double => Nbt::Double(v as f64),
        _ => Nbt::Int(v as i32),
    }
}

pub fn read(codec: &Codec, stream: &Stream, r: &mut Reader) -> Result<Nbt, String> {
    match codec {
        Codec::Normalize(inner, f) => return f(read(inner, stream, r)?),
        Codec::Alt(primary, _, _) if !matches!(stream, Stream::Either(..)) && !matches!(stream, Stream::Nbt) => return read(primary, stream, r),
        _ => {}
    }
    Ok(match stream {
        Stream::Lazy => match codec {
            Codec::Lazy(pair) => {
                let (c, s) = pair();
                read(c, s, r)?
            }
            _ => return Err("not a recursive codec".into()),
        },
        Stream::Unit => Nbt::Compound(Vec::new()),
        Stream::Bool => Nbt::Byte(rd!(r.bool()) as i8),
        Stream::Byte => canonical_number(codec, rd!(r.u8()) as i8 as i64),
        Stream::Short => canonical_number(codec, rd!(r.i16()) as i64),
        Stream::VarInt => canonical_number(codec, rd!(r.varint()) as i64),
        Stream::Int => canonical_number(codec, rd!(r.i32()) as i64),
        Stream::VarLong => canonical_number(codec, rd!(r.varlong())),
        Stream::Long => canonical_number(codec, rd!(r.i64())),
        Stream::Float => match codec {
            Codec::Double => Nbt::Double(rd!(r.f32()) as f64),
            _ => Nbt::Float(rd!(r.f32())),
        },
        Stream::Double => Nbt::Double(rd!(r.f64())),
        Stream::Utf8(max) => Nbt::String(read_utf8(r, *max)?),
        Stream::Identifier => {
            let s = read_utf8(r, 32767)?;
            Nbt::String(parse_identifier(&s).ok_or_else(|| format!("Not a valid resource location: {s}"))?)
        }
        Stream::Registry(reg) => Nbt::String(registry_entry(reg, rd!(r.varint()))?.into()),
        Stream::Holder(reg, direct_stream) => match rd!(r.varint()) {
            0 => match (codec, direct_stream) {
                (Codec::Holder(_, Some(inner)), Some(s)) => read(inner, s, r)?,
                _ => return Err("direct holder not supported".into()),
            },
            id => Nbt::String(registry_entry(reg, id - 1)?.into()),
        },
        Stream::HolderSet(reg) => {
            let n = rd!(r.varint()) - 1;
            if n == -1 {
                let tag = read_utf8(r, 32767)?;
                Nbt::String(format!("#{}", parse_identifier(&tag).unwrap_or(tag)))
            } else {
                let mut items = Vec::new();
                for _ in 0..n.max(0) {
                    items.push(Nbt::String(registry_entry(reg, rd!(r.varint()))?.into()));
                }
                if items.len() == 1 { items.pop().unwrap() } else { Nbt::List(items) }
            }
        }
        Stream::Enum => Nbt::String(codec.enum_name(rd!(r.varint())).into()),
        Stream::List(elem_stream, max) => {
            let n = read_count(r, *max)?;
            let elem = element_codec(codec);
            let mut items = Vec::with_capacity(n.min(65536));
            for _ in 0..n {
                items.push(read(elem, elem_stream, r)?);
            }
            match codec {
                Codec::Compact(_) if items.len() == 1 => items.pop().unwrap(),
                _ => Nbt::List(items),
            }
        }
        Stream::FixedList(elem_stream, size) => {
            let elem = element_codec(codec);
            let mut items = Vec::with_capacity(*size);
            for _ in 0..*size {
                items.push(read(elem, elem_stream, r)?);
            }
            Nbt::List(items)
        }
        Stream::Map(ks, vs, max) => {
            let Codec::Map(kc, vc) = codec else { return Err("not a map".into()) };
            let n = read_count(r, *max)?;
            let mut fields = Vec::with_capacity(n.min(65536));
            for _ in 0..n {
                let k = read(kc, ks, r)?;
                let v = read(vc, vs, r)?;
                let k = match k {
                    Nbt::String(k) => k,
                    _ => return Err("key is not a string".into()),
                };
                fields.retain(|(existing, _): &(String, Nbt)| *existing != k);
                fields.push((k, v));
            }
            Nbt::Compound(fields)
        }
        Stream::Record => match codec {
            Codec::Record(fields) => {
                let mut out = Vec::new();
                read_fields(fields, r, &mut out)?;
                Nbt::Compound(out)
            }
            _ => return Err("not a record".into()),
        },
        Stream::Dispatch => {
            let Codec::Dispatch(d) = codec else { return Err("not a dispatch".into()) };
            let id = rd!(r.varint());
            let index = match d.selector {
                Selector::Registry(reg) => {
                    let name = registry_entry(reg, id)?;
                    d.cases.iter().position(|(n, _)| *n == name).ok_or_else(|| format!("Unknown type {name}"))?
                }
                Selector::Enum => (id as usize).min(d.cases.len().saturating_sub(1)),
            };
            let (name, fields) = &d.cases[index];
            let mut out = vec![(d.key.to_string(), Nbt::String((*name).into()))];
            read_fields(fields, r, &mut out)?;
            Nbt::Compound(out)
        }
        Stream::Either(left, right) => {
            let (Codec::Either(a, b) | Codec::Alt(a, b, _)) = codec else { return Err("not an either".into()) };
            if rd!(r.bool()) { read(a, left, r)? } else { read(b, right, r)? }
        }
        Stream::Nbt => {
            let tag = rd!(Nbt::read(r));
            codec.decode(&tag).map_err(|e| format!("Failed to decode: {e} {}", nbt::to_string(&tag)))?
        }
        Stream::Uuid => {
            let (msb, lsb) = (rd!(r.i64()), rd!(r.i64()));
            Nbt::IntArray(vec![(msb >> 32) as i32, msb as i32, (lsb >> 32) as i32, lsb as i32])
        }
        Stream::Custom => match codec {
            Codec::Custom(c) => c.read(r)?,
            _ => return Err("no custom codec".into()),
        },
    })
}

fn read_fields(fields: &[Field], r: &mut Reader, out: &mut Vec<(String, Nbt)>) -> Result<(), String> {
    for field in fields {
        match &field.kind {
            Kind::Optional | Kind::LenientOptional => {
                if rd!(r.bool()) {
                    out.push((field.name.into(), read(&field.codec, &field.stream, r)?));
                }
            }
            Kind::Default(d) | Kind::Lenient(d) => {
                let v = read(&field.codec, &field.stream, r)?;
                if v != *d {
                    out.push((field.name.into(), v));
                }
            }
            Kind::AlwaysPresent(_) | Kind::Required | Kind::Plain => out.push((field.name.into(), read(&field.codec, &field.stream, r)?)),
        }
    }
    Ok(())
}
