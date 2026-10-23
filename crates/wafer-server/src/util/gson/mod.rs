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

fn parse_to_end(mut reader: JsonReader) -> Result<JsonElement, JsonReaderError> {
    let element = parse_reader(&mut reader)?;
    if !element.is_json_null() && reader.peek()? != JsonToken::EndDocument {
        return Err(JsonReaderError::Malformed("Did not consume the entire document.".to_string()));
    }
    Ok(element)
}

pub fn parse_lenient(json: &str) -> Result<JsonElement, JsonReaderError> {
    parse_to_end(JsonReader::new(json))
}

pub fn parse_strict(json: &str) -> Result<JsonElement, JsonReaderError> {
    let mut reader = JsonReader::new(json);
    reader.strictness = Strictness::Strict;
    parse_to_end(reader)
}

pub fn to_stable_string(element: &JsonElement) -> String {
    let mut out = String::new();
    json_writer::write_value(&mut out, element, true);
    out
}

#[cfg(test)]
mod tests;
