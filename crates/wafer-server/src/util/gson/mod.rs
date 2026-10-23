pub mod json_reader;
pub mod json_writer;

use json_reader::{JsonReader, JsonReaderError, JsonToken, Strictness};

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

fn read_element(reader: &mut JsonReader) -> Result<JsonElement, JsonReaderError> {
    match reader.peek()? {
        JsonToken::BeginArray => {
            reader.begin_array()?;
            let mut elements = Vec::new();
            while reader.has_next()? {
                elements.push(read_element(reader)?);
            }
            reader.end_array()?;
            Ok(JsonElement::Array(elements))
        }
        JsonToken::BeginObject => {
            reader.begin_object()?;
            let mut members: Vec<(String, JsonElement)> = Vec::new();
            while reader.has_next()? {
                let name = reader.next_name()?;
                let value = read_element(reader)?;
                match members.iter_mut().find(|(key, _)| *key == name) {
                    Some((_, slot)) => *slot = value,
                    None => members.push((name, value)),
                }
            }
            reader.end_object()?;
            Ok(JsonElement::Object(members))
        }
        JsonToken::String => Ok(JsonElement::String(reader.next_string()?)),
        JsonToken::Number => Ok(JsonElement::Number(reader.next_string()?)),
        JsonToken::Boolean => Ok(JsonElement::Boolean(reader.next_boolean()?)),
        JsonToken::Null => {
            reader.next_null()?;
            Ok(JsonElement::Null)
        }
        _ => unreachable!(),
    }
}

fn streams_parse(reader: &mut JsonReader) -> Result<JsonElement, JsonReaderError> {
    match reader.peek() {
        Err(JsonReaderError::EndOfInput(_)) => Ok(JsonElement::Null),
        Err(error) => Err(error),
        Ok(_) => read_element(reader),
    }
}

pub fn parse_reader(reader: &mut JsonReader) -> Result<JsonElement, JsonReaderError> {
    let strictness = reader.strictness;
    if strictness == Strictness::LegacyStrict {
        reader.strictness = Strictness::Lenient;
    }
    let result = streams_parse(reader);
    reader.strictness = strictness;
    result
}

fn exception(error: &JsonReaderError) -> String {
    match error {
        JsonReaderError::EndOfInput(message) => format!("java.io.EOFException: {message}"),
        JsonReaderError::Malformed(message) => format!("com.google.gson.stream.MalformedJsonException: {message}"),
        JsonReaderError::IllegalState(message) => format!("java.lang.IllegalStateException: {message}"),
    }
}

fn wrapped(wrapper: &str, error: &JsonReaderError) -> String {
    let cause = exception(error);
    format!("{wrapper}: {cause}\nCaused by: {cause}")
}

fn parse_to_end(mut reader: JsonReader) -> Result<JsonElement, String> {
    let element = parse_reader(&mut reader).map_err(|error| wrapped("com.google.gson.JsonSyntaxException", &error))?;
    if !element.is_json_null() {
        match reader.peek() {
            Ok(JsonToken::EndDocument) => {}
            Ok(_) => return Err("com.google.gson.JsonSyntaxException: Did not consume the entire document.".to_string()),
            Err(error @ JsonReaderError::EndOfInput(_)) => return Err(wrapped("com.google.gson.JsonIOException", &error)),
            Err(error) => return Err(wrapped("com.google.gson.JsonSyntaxException", &error)),
        }
    }
    Ok(element)
}

pub fn parse_lenient(json: &str) -> Result<JsonElement, String> {
    parse_to_end(JsonReader::new(json))
}

pub fn parse_strict(json: &str) -> Result<JsonElement, String> {
    let mut reader = JsonReader::new(json);
    reader.strictness = Strictness::Strict;
    parse_to_end(reader)
}

pub fn parse_object(json: &str) -> Result<JsonElement, String> {
    let mut reader = JsonReader::new(json);
    reader.strictness = Strictness::Strict;
    let element = read_element(&mut reader).map_err(|error| wrapped("com.google.gson.JsonParseException", &error))?;
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

pub fn to_stable_string(element: &JsonElement) -> String {
    let mut out = String::new();
    json_writer::write_value(&mut out, element, true);
    out
}

#[cfg(test)]
mod tests;
