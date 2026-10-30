use std::sync::LazyLock;

use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Reader, Writer};

use super::codec::{self, Codec, Custom, Dispatch, Selector, Stream};
use super::custom;
use super::types::{Pair, b, bool, def, identifier, lenient, lenient_opt, list, opt, record, req, string};
use crate::nbt;
use crate::network::chat::component_serialization::decode_data_source_or_none;
use crate::util::codec::DecodeResult;
use crate::util::hash_ops::{self, HashCode};

pub fn text() -> Pair {
    (Codec::Custom(Box::new(TextCodec)), Stream::Nbt)
}

pub struct TextCodec;


struct Contents {
    name: &'static str,
    fields: Codec,
}

fn arg() -> Pair {
    (Codec::Custom(Box::new(ArgCodec)), Stream::Nbt)
}

fn contents() -> Vec<Contents> {
    let r = |fields| record(fields).0;
    vec![
        Contents { name: "text", fields: r(vec![req("text", string())]) },
        Contents {
            name: "translatable",
            fields: r(vec![req("translate", string()), lenient_opt("fallback", string()), opt("with", list(arg()))]),
        },
        Contents { name: "keybind", fields: r(vec![req("keybind", string())]) },
        Contents { name: "score", fields: r(vec![req("score", record(vec![req("name", string()), req("objective", string())]))]) },
        Contents { name: "selector", fields: r(vec![req("selector", string()), opt("separator", text())]) },
        Contents {
            name: "nbt",
            fields: r(vec![
                req("nbt", string()),
                lenient("interpret", bool(), b(false)),
                lenient("plain", bool(), b(false)),
                lenient_opt("separator", text()),
            ]),
        },
        Contents { name: "object", fields: r(vec![opt("fallback", text())]) },
    ]
}

pub fn style() -> Pair {
    record(style_fields())
}

fn style_fields() -> Vec<super::codec::Field> {
    let click = Dispatch {
        key: "action",
        selector: Selector::Enum,
        cases: vec![
            ("open_url", vec![req("url", string())]),
            ("open_file", vec![req("path", string())]),
            ("run_command", vec![req("command", string())]),
            ("suggest_command", vec![req("command", string())]),
            ("show_dialog", vec![req("dialog", (Codec::AnyTag, Stream::Nbt))]),
            ("change_page", vec![req("page", (super::types::positive_int(), Stream::VarInt))]),
            ("copy_to_clipboard", vec![req("value", string())]),
            ("custom", vec![req("id", identifier()), opt("payload", (Codec::AnyTag, Stream::Nbt))]),
        ],
    };
    let mut show_item = custom::template_fields();
    show_item.truncate(3);
    let hover = Dispatch {
        key: "action",
        selector: Selector::Enum,
        cases: vec![
            ("show_text", vec![req("value", text())]),
            ("show_item", show_item),
            (
                "show_entity",
                vec![
                    req("id", (Codec::Element("minecraft:entity_type"), Stream::Registry("minecraft:entity_type"))),
                    req("uuid", custom::lenient_uuid()),
                    opt("name", text()),
                ],
            ),
        ],
    };
    vec![
        def("extra", (Codec::List(Box::new(text().0), 1, i32::MAX as usize), Stream::Nbt), Nbt::List(Vec::new())),
        opt("color", (Codec::Custom(Box::new(TextColorCodec)), Stream::Nbt)),
        opt("shadow_color", (super::types::argb_color(), Stream::Nbt)),
        opt("bold", bool()),
        opt("italic", bool()),
        opt("underlined", bool()),
        opt("strikethrough", bool()),
        opt("obfuscated", bool()),
        opt("click_event", (Codec::Dispatch(Box::new(click)), Stream::Nbt)),
        opt("hover_event", (Codec::Dispatch(Box::new(hover)), Stream::Nbt)),
        opt("insertion", string()),
        opt("font", identifier()),
    ]
}

static CONTENTS: LazyLock<Vec<Contents>> = LazyLock::new(contents);
static STYLE: LazyLock<Codec> = LazyLock::new(|| Codec::Record(style_fields()));

static UNION: LazyLock<Codec> = LazyLock::new(|| {
    let mut fields = vec![
        req("text", string()),
        req("translate", string()),
        opt("fallback", text()),
        opt("with", list(arg())),
        req("keybind", string()),
        req("score", record(vec![req("name", string()), req("objective", string())])),
        req("selector", string()),
        opt("separator", text()),
        req("nbt", string()),
        opt("interpret", bool()),
        opt("plain", bool()),
        opt("entity", string()),
        opt("block", string()),
        opt("storage", identifier()),
        opt("atlas", identifier()),
        opt("sprite", identifier()),
        opt("player", custom::profile()),
        opt("hat", bool()),
    ];
    fields.extend(style_fields());
    Codec::Record(fields)
});

fn either_error(tag: &Nbt, list_error: String, map_error: String) -> String {
    let _ = tag;
    format!("Failed to parse either. First: Failed to parse either. First: Not a string; Second: {list_error}; Second: {map_error}")
}

pub fn decode(tag: &Nbt) -> Result<Nbt, String> {
    crate::network::chat::component_serialization::decode(tag)?;
    let snbt = || nbt::to_string(tag);
    match tag {
        Nbt::String(s) => Ok(Nbt::String(s.clone())),
        Nbt::Compound(_) => decode_full(tag).map_err(|e| either_error(tag, format!("Not a list: {}", snbt()), e)),
        Nbt::List(_) | Nbt::ByteArray(_) | Nbt::IntArray(_) | Nbt::LongArray(_) => {
            let items = codec::list(tag).unwrap_or_default();
            let decoded: Result<Vec<Nbt>, String> = items.iter().map(decode).collect();
            match decoded {
                Ok(items) if items.is_empty() => Err(either_error(tag, "List must have contents".into(), format!("Not a map: {}", snbt()))),
                Ok(items) => Ok(from_list(items)),
                Err(e) => Err(either_error(tag, e, format!("Not a map: {}", snbt()))),
            }
        }
        _ => Err(either_error(tag, format!("Not a list: {}", snbt()), format!("Not a map: {}", snbt()))),
    }
}

fn from_list(mut items: Vec<Nbt>) -> Nbt {
    let rest = items.split_off(1);
    let first = items.pop().unwrap();
    if rest.is_empty() {
        return first;
    }
    let mut fields = match first {
        Nbt::String(s) => vec![("text".to_string(), Nbt::String(s))],
        Nbt::Compound(fields) => fields.into_vec(),
        other => vec![("text".to_string(), other)],
    };
    match fields.iter_mut().find(|(k, _)| k == "extra") {
        Some((_, Nbt::List(extra))) => extra.extend(rest),
        _ => fields.push(("extra".into(), Nbt::List(rest))),
    }
    Nbt::Compound(fields.into())
}

fn decode_full(tag: &Nbt) -> Result<Nbt, String> {
    let contents = match tag.get("type") {
        Some(ty) => {
            let name = codec::string(ty)?;
            let Some(c) = CONTENTS.iter().find(|c| c.name == name) else { return Err(format!("Unknown element name:{name}")) };
            c
        }
        None => CONTENTS.iter().find(|c| decode_contents(c, tag).is_ok()).ok_or_else(|| "No matching codec found".to_string())?,
    };
    let mut fields = match decode_contents(contents, tag)? {
        Nbt::Compound(fields) => fields.into_vec(),
        _ => Vec::new(),
    };
    if let Nbt::Compound(style) = STYLE.decode(tag)? {
        fields.extend(style);
    }
    fields.retain(|(k, v)| !(k == "with" && v.as_list().is_empty()));
    Ok(collapse(fields))
}

fn decode_contents(contents: &Contents, tag: &Nbt) -> Result<Nbt, String> {
    let mut value = contents.fields.decode(tag)?;
    let Nbt::Compound(fields) = &mut value else { return Ok(value) };
    match contents.name {
        "nbt" => {
            let Nbt::Compound(raw) = tag else { return Err("No matching codec found".into()) };
            let source = decode_data_source_or_none(raw).ok_or_else(|| "No matching codec found".to_string())?;
            if fields.iter().any(|(k, v)| k == "interpret" && *v == Nbt::Byte(1)) && fields.iter().any(|(k, v)| k == "plain" && *v == Nbt::Byte(1)) {
                return Err("'interpret' and 'plain' flags can't be both on".into());
            }
            let (key, value) = source.to_field();
            fields.push((key.into(), value));
        }
        "object" => {
            let object: Vec<(String, Nbt)> = if tag.get("sprite").is_some() {
                let atlas = match tag.get("atlas") {
                    Some(a) => Some(codec::identifier(a)?),
                    None => None,
                };
                let mut out = vec![("sprite".to_string(), Nbt::String(codec::identifier(tag.get("sprite").unwrap())?))];
                if let Some(atlas) = atlas.filter(|a| a != "minecraft:blocks") {
                    out.push(("atlas".into(), Nbt::String(atlas)));
                }
                out
            } else if let Some(player) = tag.get("player") {
                let mut out = vec![("player".to_string(), custom::profile().0.decode(player)?)];
                if let Some(hat) = tag.get("hat") {
                    let hat = Codec::Bool.decode(hat)?;
                    if hat != b(true) {
                        out.push(("hat".into(), hat));
                    }
                }
                out
            } else {
                return Err("No matching codec found".into());
            };
            fields.extend(object);
        }
        _ => {}
    }
    Ok(value)
}

fn collapse(fields: Vec<(String, Nbt)>) -> Nbt {
    if let [(key, Nbt::String(text))] = fields.as_slice()
        && key == "text"
    {
        return Nbt::String(text.clone());
    }
    Nbt::Compound(fields.into())
}

pub fn hash(value: &Nbt) -> HashCode {
    match value {
        Nbt::String(s) => hash_ops::create_string(s),
        other => UNION.hash(other),
    }
}

pub fn decode_partial(tag: &Nbt) -> DecodeResult<Nbt> {
    let component = crate::network::chat::component_serialization::decode_partial(tag);
    match component.error {
        None => DecodeResult::from(decode(tag)),
        Some(error) => DecodeResult { value: component.value.map(|component| component.to_nbt()), error: Some(error) },
    }
}

impl Custom for TextCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        decode(tag)
    }

    fn decode_partial(&self, tag: &Nbt) -> DecodeResult<Nbt> {
        decode_partial(tag)
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        hash(value)
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        value.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        let tag = Nbt::read(r).map_err(|e| e.to_string())?;
        decode(&tag)
    }
}

struct ArgCodec;

fn java_ops_number(tag: &Nbt) -> Nbt {
    let integral = |v: i64| {
        if let Ok(v) = i8::try_from(v) {
            Nbt::Byte(v)
        } else if let Ok(v) = i16::try_from(v) {
            Nbt::Short(v)
        } else if let Ok(v) = i32::try_from(v) {
            Nbt::Int(v)
        } else {
            Nbt::Long(v)
        }
    };
    match *tag {
        Nbt::LazilyParsedNumber(ref text) => wafer_protocol::lazily_parsed_number::exact_nbt(text),
        Nbt::Long(v) => integral(v),
        Nbt::Double(v) if v.fract() == 0.0 && (-9.223372036854775808E18..9.223372036854775808E18).contains(&v) => integral(v as i64),
        Nbt::Double(v) if v as f32 as f64 == v => Nbt::Float(v as f32),
        _ => tag.clone(),
    }
}

fn json_null_containers(tag: &Nbt, containers: &mut Vec<bool>) -> bool {
    let (children, is_list): (Vec<&Nbt>, bool) = match tag {
        Nbt::List(items) => (items.iter().collect(), true),
        Nbt::Compound(fields) => (fields.iter().map(|(_, value)| value).collect(), false),
        _ => return false,
    };
    containers.push(is_list);
    for child in children {
        if matches!(child, Nbt::End) || json_null_containers(child, containers) {
            return true;
        }
    }
    containers.pop();
    false
}

const JSON_OPS_NULL_POINTER: &str = "Cannot invoke \"com.google.gson.JsonElement.getAsJsonPrimitive()\" because \"input\" is null";

const JSON_OPS_LEAF: [&str; 2] = ["com.mojang.serialization.JsonOps.convertTo(JsonOps.java:61)", "com.mojang.serialization.JsonOps.convertTo(JsonOps.java:25)"];

const JSON_OPS_LIST_LEVEL: [&str; 14] = [
    "com.mojang.serialization.DynamicOps.lambda$convertList$31(DynamicOps.java:245)",
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "java.base/java.util.ArrayList$Itr.forEachRemaining(ArrayList.java:1086)",
    "java.base/java.util.Spliterators$IteratorSpliterator.forEachRemaining(Spliterators.java:1939)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:570)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:635)",
    "java.base/java.util.stream.AbstractPipeline.evaluateToArrayNode(AbstractPipeline.java:291)",
    "java.base/java.util.stream.ReferencePipeline.toArray(ReferencePipeline.java:652)",
    "java.base/java.util.stream.ReferencePipeline.toArray(ReferencePipeline.java:658)",
    "java.base/java.util.stream.ReferencePipeline.toList(ReferencePipeline.java:663)",
    "com.mojang.serialization.JavaOps.createList(JavaOps.java:345)",
    "com.mojang.serialization.DynamicOps.convertList(DynamicOps.java:245)",
];

const JSON_OPS_MAP_LEVEL: [&str; 12] = [
    "com.mojang.serialization.DynamicOps.lambda$convertMap$32(DynamicOps.java:250)",
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "java.base/java.util.stream.ReferencePipeline$3$1.accept(ReferencePipeline.java:214)",
    "java.base/java.util.Iterator.forEachRemaining(Iterator.java:133)",
    "java.base/java.util.Spliterators$IteratorSpliterator.forEachRemaining(Spliterators.java:1939)",
    "java.base/java.util.stream.AbstractPipeline.copyInto(AbstractPipeline.java:570)",
    "java.base/java.util.stream.AbstractPipeline.wrapAndCopyInto(AbstractPipeline.java:560)",
    "java.base/java.util.stream.ReduceOps$ReduceOp.evaluateSequential(ReduceOps.java:921)",
    "java.base/java.util.stream.AbstractPipeline.evaluate(AbstractPipeline.java:265)",
    "java.base/java.util.stream.ReferencePipeline.collect(ReferencePipeline.java:723)",
    "com.mojang.serialization.JavaOps.createMap(JavaOps.java:287)",
    "com.mojang.serialization.DynamicOps.convertMap(DynamicOps.java:249)",
];

const JSON_OPS_ARG_CONVERTER: [&str; 11] = [
    "net.minecraft.resources.DelegatingOps.convertTo(DelegatingOps.java:52)",
    "com.mojang.serialization.Dynamic.convert(Dynamic.java:231)",
    "com.mojang.serialization.Dynamic.convert(Dynamic.java:213)",
    "net.minecraft.util.ExtraCodecs.lambda$converter$0(ExtraCodecs.java:90)",
    "com.mojang.datafixers.util.Pair.mapFirst(Pair.java:64)",
    "com.mojang.serialization.Decoder$2.lambda$decode$0(Decoder.java:63)",
    "com.mojang.serialization.DataResult$Success.map(DataResult.java:175)",
    "com.mojang.serialization.Decoder$2.decode(Decoder.java:63)",
    "com.mojang.serialization.Codec$2.decode(Codec.java:74)",
    "com.mojang.serialization.Decoder$1.decode(Decoder.java:49)",
    "com.mojang.serialization.Codec$2.decode(Codec.java:74)",
];

pub fn json_ops_null_pointer(tag: &Nbt) -> Option<String> {
    if !matches!(tag, Nbt::List(_) | Nbt::Compound(_)) || !nbt::is_json_source() {
        return None;
    }
    let mut containers = Vec::new();
    if !json_null_containers(tag, &mut containers) {
        return None;
    }
    let mut frames: Vec<&'static str> = JSON_OPS_LEAF.to_vec();
    for is_list in containers.iter().rev() {
        let level: &[&'static str] = if *is_list { &JSON_OPS_LIST_LEVEL } else { &JSON_OPS_MAP_LEVEL };
        frames.extend_from_slice(level);
        frames.push(if *is_list { "com.mojang.serialization.JsonOps.convertTo(JsonOps.java:56)" } else { "com.mojang.serialization.JsonOps.convertTo(JsonOps.java:53)" });
        frames.push(JSON_OPS_LEAF[1]);
    }
    frames.extend_from_slice(&JSON_OPS_ARG_CONVERTER);
    crate::core::data_registries::throw_unwinding("java.lang.NullPointerException", JSON_OPS_NULL_POINTER.into(), &frames);
    Some(format!("java.lang.NullPointerException: {JSON_OPS_NULL_POINTER}"))
}

fn json_ops_number_format(tag: &Nbt) -> Option<String> {
    let Nbt::LazilyParsedNumber(text) = tag else { return None };
    if !nbt::is_json_source() {
        return None;
    }
    let message = wafer_protocol::lazily_parsed_number::number_limits_error(text)?;
    let mut frames: Vec<&'static str> = vec![
        "com.google.gson.internal.NumberLimits.parseBigDecimal(NumberLimits.java:27)",
        "com.google.gson.JsonPrimitive.getAsBigDecimal(JsonPrimitive.java:185)",
        "com.mojang.serialization.JsonOps.convertTo(JsonOps.java:68)",
        JSON_OPS_LEAF[1],
    ];
    frames.extend_from_slice(&JSON_OPS_ARG_CONVERTER);
    crate::core::data_registries::throw_unwinding("java.lang.NumberFormatException", message.clone(), &frames);
    Some(format!("java.lang.NumberFormatException: {message}"))
}

pub fn json_ops_failure(tag: &Nbt) -> Option<String> {
    json_ops_null_pointer(tag).or_else(|| json_ops_number_format(tag))
}

impl Custom for ArgCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        if let Some(message) = json_ops_failure(tag) {
            return Err(message);
        }
        match tag {
            Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) | Nbt::LazilyParsedNumber(_) if nbt::is_json_source() => Ok(java_ops_number(tag)),
            Nbt::Byte(_) | Nbt::Short(_) | Nbt::Int(_) | Nbt::Long(_) | Nbt::Float(_) | Nbt::Double(_) | Nbt::String(_) => Ok(tag.clone()),
            other => decode(other).map_err(|e| format!("Failed to parse either. First: This value needs to be parsed as component; Second: {e}")),
        }
    }

    fn hash(&self, value: &Nbt) -> HashCode {
        match value {
            Nbt::Compound(_) | Nbt::List(_) => hash(value),
            other => hash_ops::hash_nbt(other),
        }
    }

    fn write(&self, value: &Nbt, w: &mut Writer) -> Result<(), String> {
        value.write(w);
        Ok(())
    }

    fn read(&self, r: &mut Reader) -> Result<Nbt, String> {
        Nbt::read(r).map_err(|e| e.to_string())
    }
}

pub const NAMED_COLORS: [(&str, i32); 16] = [
    ("black", 0),
    ("dark_blue", 170),
    ("dark_green", 43520),
    ("dark_aqua", 43690),
    ("dark_red", 11141120),
    ("dark_purple", 11141290),
    ("gold", 16755200),
    ("gray", 11184810),
    ("dark_gray", 5592405),
    ("blue", 5592575),
    ("green", 5635925),
    ("aqua", 5636095),
    ("red", 16733525),
    ("light_purple", 16733695),
    ("yellow", 16777045),
    ("white", 16777215),
];

struct TextColorCodec;

impl Custom for TextColorCodec {
    fn decode(&self, tag: &Nbt) -> Result<Nbt, String> {
        let color = codec::string(tag)?;
        if let Some(hex) = color.strip_prefix('#') {
            let parsed = match hex.strip_prefix('+') {
                Some(rest) if !rest.starts_with(['+', '-']) => i64::from_str_radix(rest, 16),
                _ => i64::from_str_radix(hex, 16),
            };
            match parsed {
                Ok(v) if (i32::MIN as i64..=i32::MAX as i64).contains(&v) => {
                    if (0..=0xFFFFFF).contains(&v) {
                        Ok(Nbt::String(format!("#{v:06X}")))
                    } else {
                        Err(format!("Color value out of range: {color}"))
                    }
                }
                _ => Err(format!("Invalid color value: {color}")),
            }
        } else if NAMED_COLORS.iter().any(|(n, _)| *n == color) {
            Ok(Nbt::String(color.into()))
        } else {
            Err(format!("Invalid color name: {color}"))
        }
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

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(snbt: &str) -> Nbt {
        crate::nbt::parse_fully(snbt).unwrap()
    }

    #[test]
    fn plain_text_collapses() {
        assert_eq!(decode(&parse("{text:\"a\"}")), Ok(Nbt::String("a".into())));
        assert_eq!(decode(&parse("{type:\"text\",text:\"a\"}")), Ok(Nbt::String("a".into())));
        assert_eq!(decode(&parse("[\"a\"]")), Ok(Nbt::String("a".into())));
        assert_eq!(decode(&parse("[\"a\",\"b\"]")), Ok(parse("{text:\"a\",extra:[\"b\"]}")));
        assert_eq!(decode(&parse("{text:\"a\",bold:1}")), Ok(parse("{text:\"a\",bold:1b}")));
        assert_eq!(decode(&parse("{translate:\"x\",with:[]}")), Ok(parse("{translate:\"x\"}")));
        assert_eq!(decode(&parse("{text:\"a\",color:\"#ff0000\"}")), Ok(parse("{text:\"a\",color:\"#FF0000\"}")));
        assert!(decode(&parse("{text:\"a\",extra:[]}")).is_err());
        assert!(decode(&parse("{}")).is_err());
    }

    #[test]
    fn an_invalid_color_keeps_the_rest_of_the_component() {
        let result = decode_partial(&parse("{text:\"a\",extra:[{text:\"b\",color:\"reset\",bold:1b}]}"));
        assert_eq!(result.error.as_deref(), Some("Invalid color name: reset"));
        assert_eq!(result.value, Some(parse("{text:\"a\",extra:[{text:\"b\",bold:1b}]}")));
    }

    #[test]
    fn a_component_with_an_invalid_color_stays_in_the_patch() {
        let patch = crate::world::item::component::DataComponentPatch::from_nbt_partial(&parse("{\"minecraft:custom_name\":{text:\"a\",color:\"reset\"}}"));
        assert!(patch.error.is_some());
        assert_eq!(patch.value.expect("a partial patch").to_nbt(), parse("{\"minecraft:custom_name\":\"a\"}"));
    }
}
