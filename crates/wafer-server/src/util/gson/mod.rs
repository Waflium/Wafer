pub mod json_reader;
pub mod json_writer;

use crate::util::java_throwable::JavaThrowable;
use json_reader::{JsonReader, JsonReaderError, JsonReaderErrorKind, JsonToken, Strictness, Traced};

#[derive(Clone, Debug, PartialEq)]
pub enum JsonElement {
    Null,
    Boolean(bool),
    Number(String),
    String(String),
    Array(Vec<JsonElement>),
    Object(Vec<(String, JsonElement)>),
}

impl JsonElement {
    pub fn is_json_null(&self) -> bool {
        matches!(self, JsonElement::Null)
    }

    pub fn is_json_primitive(&self) -> bool {
        matches!(self, JsonElement::Boolean(_) | JsonElement::Number(_) | JsonElement::String(_))
    }

    pub fn get_as_string(&self) -> String {
        match self {
            JsonElement::Boolean(value) => value.to_string(),
            JsonElement::Number(value) | JsonElement::String(value) => value.clone(),
            other => panic!("{other:?} is not a primitive"),
        }
    }

    pub fn get(&self, name: &str) -> Option<&JsonElement> {
        match self {
            JsonElement::Object(members) => members.iter().find(|(key, _)| key == name).map(|(_, value)| value),
            _ => None,
        }
    }
}

macro_rules! adapter {
    ($method:literal, $line:literal) => {
        concat!("com.google.gson.internal.bind.JsonElementTypeAdapter.", $method, "(JsonElementTypeAdapter.java:", $line, ")")
    };
}

macro_rules! streams {
    ($line:literal) => {
        concat!("com.google.gson.internal.Streams.parse(Streams.java:", $line, ")")
    };
}

macro_rules! strict_json_parser {
    ($line:literal) => {
        concat!("net.minecraft.util.StrictJsonParser.parse(StrictJsonParser.java:", $line, ")")
    };
}

const JSON_PARSER_PARSE_READER: &str = "com.google.gson.JsonParser.parseReader(JsonParser.java:144)";

fn read_element(reader: &mut JsonReader, nested: bool) -> Result<JsonElement, JsonReaderError> {
    let (read_peek, read_nesting, read_terminal) = if nested {
        (adapter!("read", 102), adapter!("read", 103), adapter!("read", 107))
    } else {
        (adapter!("read", 85), adapter!("read", 87), adapter!("read", 89))
    };
    match reader.peek().at(read_peek)? {
        JsonToken::BeginArray => {
            reader.begin_array().at(adapter!("tryBeginNesting", 47)).at(read_nesting)?;
            let mut elements = Vec::new();
            while reader.has_next().at(adapter!("read", 95))? {
                elements.push(read_element(reader, true)?);
            }
            reader.end_array().at(adapter!("read", 124))?;
            Ok(JsonElement::Array(elements))
        }
        JsonToken::BeginObject => {
            reader.begin_object().at(adapter!("tryBeginNesting", 50)).at(read_nesting)?;
            let mut members: Vec<(String, JsonElement)> = Vec::new();
            while reader.has_next().at(adapter!("read", 95))? {
                let name = reader.next_name().at(adapter!("read", 99))?;
                let value = read_element(reader, true)?;
                match members.iter_mut().find(|(key, _)| *key == name) {
                    Some((_, slot)) => *slot = value,
                    None => members.push((name, value)),
                }
            }
            reader.end_object().at(adapter!("read", 126))?;
            Ok(JsonElement::Object(members))
        }
        JsonToken::String => Ok(JsonElement::String(reader.next_string().at(adapter!("readTerminal", 61)).at(read_terminal)?)),
        JsonToken::Number => Ok(JsonElement::Number(reader.next_string().at(adapter!("readTerminal", 63)).at(read_terminal)?)),
        JsonToken::Boolean => Ok(JsonElement::Boolean(reader.next_boolean().at(adapter!("readTerminal", 66)).at(read_terminal)?)),
        JsonToken::Null => {
            reader.next_null().at(adapter!("readTerminal", 68)).at(read_terminal)?;
            Ok(JsonElement::Null)
        }
        _ => unreachable!(),
    }
}

fn streams_parse(reader: &mut JsonReader) -> Result<JsonElement, JavaThrowable> {
    match reader.peek().at(streams!(46)) {
        Err(error) if error.kind == JsonReaderErrorKind::EndOfInput => Ok(JsonElement::Null),
        Err(error) => Err(wrap_streams_error(error)),
        Ok(_) => read_element(reader, false).at(streams!(48)).map_err(wrap_streams_error),
    }
}

fn exception_class(kind: JsonReaderErrorKind) -> &'static str {
    match kind {
        JsonReaderErrorKind::EndOfInput => "java.io.EOFException",
        JsonReaderErrorKind::Malformed => "com.google.gson.stream.MalformedJsonException",
        JsonReaderErrorKind::IllegalState => "java.lang.IllegalStateException",
    }
}

fn to_throwable(error: JsonReaderError, outer: &[&'static str]) -> JavaThrowable {
    let mut stack_trace = error.stack_trace;
    stack_trace.extend_from_slice(outer);
    JavaThrowable::new(format!("{}: {}", exception_class(error.kind), error.message), stack_trace)
}

fn wrap_streams_error(error: JsonReaderError) -> JavaThrowable {
    let outer = [JSON_PARSER_PARSE_READER, strict_json_parser!(27)];
    if error.kind == JsonReaderErrorKind::IllegalState {
        return to_throwable(error, &outer);
    }
    let line = if error.kind == JsonReaderErrorKind::EndOfInput { streams!(58) } else { streams!(60) };
    wrap_in_exception("com.google.gson.JsonSyntaxException", to_throwable(error, &outer), vec![line, JSON_PARSER_PARSE_READER, strict_json_parser!(27)])
}

fn wrap_in_exception(class: &str, cause: JavaThrowable, stack_trace: Vec<&'static str>) -> JavaThrowable {
    JavaThrowable::new(format!("{class}: {}", cause.description), stack_trace).with_cause(cause)
}

fn parse_reader(reader: &mut JsonReader) -> Result<JsonElement, JavaThrowable> {
    let strictness = reader.strictness;
    if strictness == Strictness::LegacyStrict {
        reader.strictness = Strictness::Lenient;
    }
    let result = streams_parse(reader);
    reader.strictness = strictness;
    result
}

fn parse_to_end(mut reader: JsonReader) -> Result<JsonElement, JavaThrowable> {
    let element = parse_reader(&mut reader)?;
    if !element.is_json_null() {
        match reader.peek().at(strict_json_parser!(28)) {
            Ok(JsonToken::EndDocument) => {}
            Ok(_) => return Err(JavaThrowable::new("com.google.gson.JsonSyntaxException: Did not consume the entire document.", vec![strict_json_parser!(29)])),
            Err(error) if error.kind == JsonReaderErrorKind::EndOfInput => {
                return Err(wrap_in_exception("com.google.gson.JsonIOException", to_throwable(error, &[]), vec![strict_json_parser!(35)]));
            }
            Err(error) => return Err(wrap_in_exception("com.google.gson.JsonSyntaxException", to_throwable(error, &[]), vec![strict_json_parser!(33)])),
        }
    }
    Ok(element)
}

fn chain_description(throwable: &JavaThrowable) -> String {
    let mut chain = throwable.description.clone();
    let mut cause = throwable.cause.as_deref();
    while let Some(next) = cause {
        chain.push_str("\nCaused by: ");
        chain.push_str(&next.description);
        cause = next.cause.as_deref();
    }
    chain
}

pub fn parse_lenient(json: &str) -> Result<JsonElement, String> {
    parse_to_end(JsonReader::new(json)).map_err(|error| chain_description(&error))
}

pub fn parse_strict(json: &str) -> Result<JsonElement, String> {
    parse_strict_throwable(json).map_err(|error| chain_description(&error))
}

pub fn parse_strict_throwable(json: &str) -> Result<JsonElement, JavaThrowable> {
    let mut reader = JsonReader::new(json);
    reader.strictness = Strictness::Strict;
    parse_to_end(reader)
}

pub fn parse_object(json: &str) -> Result<JsonElement, String> {
    let mut reader = JsonReader::new(json);
    reader.strictness = Strictness::Strict;
    let element = read_element(&mut reader, false).map_err(|error| {
        let cause = to_throwable(error, &[]);
        chain_description(&wrap_in_exception("com.google.gson.JsonParseException", cause, Vec::new()))
    })?;
    let class = match element {
        JsonElement::Object(_) => return Ok(element),
        JsonElement::Null => "JsonNull",
        JsonElement::Array(_) => "JsonArray",
        _ => "JsonPrimitive",
    };
    Err(format!("com.google.gson.JsonSyntaxException: Expected a com.google.gson.JsonObject but was com.google.gson.{class}; at path {}", reader.get_previous_path()))
}

pub fn to_json_string(element: &JsonElement) -> String {
    let mut out = String::new();
    json_writer::write_value(&mut out, element, false);
    out
}

pub fn to_value(element: JsonElement) -> serde_json::Value {
    match element {
        JsonElement::Null => serde_json::Value::Null,
        JsonElement::Boolean(value) => serde_json::Value::Bool(value),
        JsonElement::Number(text) => number(&text),
        JsonElement::String(value) => serde_json::Value::String(value),
        JsonElement::Array(elements) => serde_json::Value::Array(elements.into_iter().map(to_value).collect()),
        JsonElement::Object(members) => serde_json::Value::Object(members.into_iter().map(|(name, value)| (name, to_value(value))).collect()),
    }
}

fn number(text: &str) -> serde_json::Value {
    if let Ok(value) = text.parse::<i64>() {
        return serde_json::Value::Number(value.into());
    }
    if let Ok(value) = text.parse::<u64>() {
        return serde_json::Value::Number(value.into());
    }
    text.parse::<f64>().ok().and_then(serde_json::Number::from_f64).map_or(serde_json::Value::Null, serde_json::Value::Number)
}

pub fn parse_lenient_value(json: &str) -> Result<serde_json::Value, String> {
    parse_lenient(json).map(to_value)
}

pub fn parse_strict_value(json: &str) -> Result<serde_json::Value, String> {
    parse_strict(json).map(to_value)
}

pub fn parse_strict_bytes(bytes: &[u8]) -> Result<serde_json::Value, String> {
    parse_strict_value(&String::from_utf8_lossy(bytes))
}

pub fn parse_strict_bytes_throwable(bytes: &[u8]) -> Result<serde_json::Value, JavaThrowable> {
    parse_strict_throwable(&String::from_utf8_lossy(bytes)).map(to_value)
}

pub fn to_stable_string(element: &JsonElement) -> String {
    let mut out = String::new();
    json_writer::write_value(&mut out, element, true);
    out
}

#[cfg(test)]
mod tests;
