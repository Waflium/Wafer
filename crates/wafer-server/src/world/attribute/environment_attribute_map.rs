use std::sync::LazyLock;

use serde_json::Value as Json;
use wafer_protocol::nbt::Nbt;

use super::attribute_type::{rgb_color_codec, argb_color_codec};
use super::environment_attribute::EnvironmentAttributeInfo;
use super::environment_attributes::ATTRIBUTES;
use super::modifier::{AttributeModifier, OperationId};
use super::{AttributeRange, AttributeType, EnvironmentAttribute, Value};
use crate::util::codec::DecodeResult;
use crate::world::item::component::codec::{Codec, Custom, Dispatch, Field, Kind, Selector, Stream, Wrap};
use crate::world::item::component::types::{self, Pair};
use crate::world::level::levelgen::density_function::resource_name;
use wafer_protocol::{Reader, Writer};

use crate::util::hash_ops::{self, HashCode};

struct Validated {
    inner: Codec,
    range: AttributeRange,
}

impl Custom for Validated {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        self.decode_partial(tag).into_result()
    }

    fn decode_partial(&self, tag: &Nbt) -> DecodeResult<Nbt> {
        self.inner.decode_partial(tag).validate(|value| match value {
            Nbt::Float(float) => self.range.validate(&Value::Float(*float)),
            _ => Ok(()),
        })
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash_ops::hash_nbt(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        value.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Nbt::read(r).map_err(|e| e.to_string())
    }
}

fn value_codec(attribute: &EnvironmentAttributeInfo) -> Codec {
    let inner = attribute.ty.condition_value_codec().0;
    match attribute.value_range {
        AttributeRange::Any => inner,
        range => Codec::Custom(Box::new(Validated { inner, range })),
    }
}

fn float_with_alpha_codec() -> Codec {
    let full = types::record(vec![types::req("value", types::float(Codec::float())), types::def("alpha", types::float(types::codec_float_range(0.0, 1.0)), Nbt::Float(1.0))]);
    Codec::Either(Box::new(Codec::float()), Box::new(full.0))
}

fn blend_to_gray_codec() -> Codec {
    types::record(vec![types::req("brightness", types::float(types::codec_float_range(0.0, 1.0))), types::req("factor", types::float(types::codec_float_range(0.0, 1.0)))]).0
}

pub(crate) fn argument_codec(attribute: &EnvironmentAttributeInfo, operation: OperationId) -> Codec {
    use OperationId as Op;
    match (attribute.ty, operation) {
        (_, Op::Override | Op::Append | Op::Overlay) => value_codec(attribute),
        (AttributeType::Boolean, _) => Codec::Bool,
        (AttributeType::Float | AttributeType::AngleDegrees, Op::AlphaBlend) => float_with_alpha_codec(),
        (AttributeType::Float | AttributeType::AngleDegrees, _) => Codec::float(),
        (AttributeType::Integer, _) => Codec::int(),
        (AttributeType::RgbColor | AttributeType::ArgbColor, Op::BlendToGray) => blend_to_gray_codec(),
        (AttributeType::RgbColor, Op::AlphaBlend) => argb_color_codec(),
        (AttributeType::RgbColor, _) => rgb_color_codec(),
        (AttributeType::ArgbColor, Op::AlphaBlend | Op::Multiply) => argb_color_codec(),
        (AttributeType::ArgbColor, _) => rgb_color_codec(),
        _ => value_codec(attribute),
    }
}

pub(crate) const OPERATION_NAMES: [(&str, &str); 16] = [
    ("override", "OVERRIDE"),
    ("alpha_blend", "ALPHA_BLEND"),
    ("add", "ADD"),
    ("subtract", "SUBTRACT"),
    ("multiply", "MULTIPLY"),
    ("blend_to_gray", "BLEND_TO_GRAY"),
    ("minimum", "MINIMUM"),
    ("maximum", "MAXIMUM"),
    ("and", "AND"),
    ("nand", "NAND"),
    ("or", "OR"),
    ("nor", "NOR"),
    ("xor", "XOR"),
    ("xnor", "XNOR"),
    ("append", "APPEND"),
    ("overlay", "OVERLAY"),
];

fn full_codec(attribute: &EnvironmentAttributeInfo) -> Codec {
    let cases = OperationId::VALUES
        .into_iter()
        .filter(|&operation| attribute.ty.modifier(operation).is_some())
        .map(|operation| (operation.name(), vec![Field::new("argument", argument_codec(attribute, operation), Stream::Nbt, Kind::Required)]))
        .collect();
    Codec::Dispatch(Box::new(Dispatch { key: "modifier", selector: Selector::Resolved(&OPERATION_NAMES), cases }))
}

fn entry_codec(attribute: &EnvironmentAttributeInfo) -> Codec {
    Codec::Framed(Box::new(Codec::Either(Box::new(value_codec(attribute)), Box::new(full_codec(attribute)))), Wrap::Xmap)
}

static ENTRY_CODECS: LazyLock<Vec<Codec>> = LazyLock::new(|| ATTRIBUTES.iter().map(entry_codec).collect());

fn entry_codec_of(key: &Nbt) -> Option<&'static Codec> {
    let attribute = EnvironmentAttributeInfo::by_name(key.as_str()?)?;
    ENTRY_CODECS.get(attribute.id)
}

pub fn codec() -> Pair {
    let dispatched = Codec::DispatchedMap(Box::new(types::by_name("minecraft:environment_attribute").0), entry_codec_of);
    (Codec::Framed(Box::new(Codec::Framed(Box::new(dispatched), Wrap::Xmap)), Wrap::Recursive), Stream::Nbt)
}

pub fn decode_only_positional(tag: &Nbt) -> DecodeResult<Nbt> {
    crate::util::codec::wrapped(Wrap::FlatXmap, || {
        codec().0.decode_partial(tag).validate(|map| {
            let Nbt::Compound(entries) = map else { return Ok(()) };
            let illegal: Vec<&str> = entries.iter().map(|(name, _)| name.as_str()).filter(|name| EnvironmentAttributeInfo::by_name(name).is_some_and(|attribute| !attribute.is_positional)).collect();
            if illegal.is_empty() { Ok(()) } else { Err(format!("The following attributes cannot be positional: [{}]", illegal.join(", "))) }
        })
    })
}

#[derive(Clone, PartialEq, Debug)]
pub struct Entry {
    pub argument: Value,
    pub modifier: AttributeModifier,
}

impl Entry {
    pub fn apply_modifier(&self, subject: &Value) -> Value {
        self.modifier.apply(subject, &self.argument)
    }

    pub fn encode(&self, attribute: &EnvironmentAttributeInfo) -> Result<Nbt, String> {
        if self.modifier == AttributeModifier::Override {
            return attribute.ty.encode_value(&self.argument);
        }
        Ok(Nbt::Compound(vec![
            ("modifier".into(), Nbt::String(self.modifier.operation_id().name().into())),
            ("argument".into(), self.modifier.encode_argument(attribute, &self.argument)?),
        ].into()))
    }

    fn parse(attribute: &EnvironmentAttributeInfo, json: &Json) -> Result<Entry, String> {
        Entry::parse_with_form(attribute, json).map(|(entry, _)| entry)
    }

    pub(crate) fn parse_with_form(attribute: &EnvironmentAttributeInfo, json: &Json) -> Result<(Entry, bool), String> {
        let value_error = match attribute.parse_value(json) {
            Ok(argument) => return Ok((Entry { argument, modifier: AttributeModifier::Override }, false)),
            Err(e) => e,
        };
        let full = || -> Result<(Entry, bool), String> {
            let operation = json.get("modifier").and_then(Json::as_str).ok_or("missing modifier")?;
            let modifier = OperationId::by_name(operation)
                .and_then(|op| attribute.ty.modifier(op))
                .ok_or_else(|| format!("unknown modifier {operation} for {}", attribute.name))?;
            let argument = modifier.parse_argument(attribute, json.get("argument").ok_or("missing argument")?)?;
            Ok((Entry { argument, modifier }, true))
        };
        full().map_err(|e| format!("{}: {value_error}; {e}", attribute.name))
    }
}

#[derive(Clone, PartialEq, Debug, Default)]
pub struct EnvironmentAttributeMap {
    entries: Vec<(usize, Entry)>,
}

impl EnvironmentAttributeMap {
    pub fn parse(json: &Json) -> EnvironmentAttributeMap {
        let mut map = EnvironmentAttributeMap::default();
        let Some(object) = json.as_object() else { return map };
        for (name, value) in object {
            let Some(attribute) = EnvironmentAttributeInfo::by_name(&resource_name(name)) else { continue };
            if let Ok(entry) = Entry::parse(attribute, value) {
                map.put(attribute.id, entry);
            }
        }
        map
    }

    pub fn encode_network(&self) -> Result<Nbt, String> {
        Ok(Nbt::Compound(
            self.entries
                .iter()
                .filter(|(id, _)| ATTRIBUTES[*id].is_syncable)
                .map(|(id, entry)| Ok((ATTRIBUTES[*id].name.to_string(), entry.encode(&ATTRIBUTES[*id])?)))
                .collect::<Result<_, String>>()?,
        ))
    }

    fn put(&mut self, id: usize, entry: Entry) {
        match self.entries.iter_mut().find(|(i, _)| *i == id) {
            Some(slot) => slot.1 = entry,
            None => self.entries.push((id, entry)),
        }
    }

    pub fn modify<T>(mut self, attribute: EnvironmentAttribute<T>, modifier: AttributeModifier, argument: Value) -> Self {
        self.put(attribute.id(), Entry { argument, modifier });
        self
    }

    pub fn set<T>(self, attribute: EnvironmentAttribute<T>, value: Value) -> Self {
        self.modify(attribute, AttributeModifier::Override, value)
    }

    pub fn get(&self, id: usize) -> Option<&Entry> {
        self.entries.iter().find(|(i, _)| *i == id).map(|(_, e)| e)
    }

    pub fn contains(&self, id: usize) -> bool {
        self.get(id).is_some()
    }

    pub fn key_set(&self) -> impl Iterator<Item = usize> + '_ {
        self.entries.iter().map(|(i, _)| *i)
    }

    pub fn entries(&self) -> &[(usize, Entry)] {
        &self.entries
    }
}
