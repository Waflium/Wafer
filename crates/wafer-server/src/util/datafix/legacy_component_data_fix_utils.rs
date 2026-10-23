use wafer_protocol::nbt::Nbt;

use crate::datafixers::dynamic::Dynamic;
use crate::util::gson::{self, JsonElement};

const EMPTY_CONTENTS: &str = "{\"text\":\"\"}";

pub fn create_plain_text_component(text: &str) -> Dynamic {
    Dynamic::new(Nbt::String(create_text_component_json(text)))
}

pub fn create_empty_component() -> Dynamic {
    Dynamic::new(Nbt::String(EMPTY_CONTENTS.to_string()))
}

pub fn create_text_component_json(text: &str) -> String {
    gson::to_stable_string(&JsonElement::Object(vec![("text".to_string(), JsonElement::String(text.to_string()))]))
}

pub fn create_translatable_component_json(key: &str) -> String {
    gson::to_stable_string(&JsonElement::Object(vec![("translate".to_string(), JsonElement::String(key.to_string()))]))
}

pub fn create_translatable_component(key: &str) -> Dynamic {
    Dynamic::new(Nbt::String(create_translatable_component_json(key)))
}

pub fn rewrite_from_lenient(string: &str) -> String {
    if string.is_empty() || string == "null" {
        return EMPTY_CONTENTS.to_string();
    }
    let first_char = string.encode_utf16().next();
    let last_char = string.encode_utf16().last();
    let quoted = |first: u8, last: u8| first_char == Some(first as u16) && last_char == Some(last as u16);
    if quoted(b'"', b'"') || quoted(b'{', b'}') || quoted(b'[', b']') {
        if let Ok(json) = gson::parse_lenient(string) {
            if json.is_json_primitive() {
                return create_text_component_json(&json.get_as_string());
            }
            return gson::to_stable_string(&json);
        }
    }
    create_text_component_json(string)
}

pub fn is_strictly_valid_json(component: &Dynamic) -> bool {
    component.as_string().into_result().is_ok_and(|string| gson::parse_strict(&string).is_ok())
}

pub fn extract_translation_string(component: &str) -> Option<String> {
    match gson::parse_lenient(component) {
        Ok(parsed @ JsonElement::Object(_)) => parsed.get("translate").filter(|key| key.is_json_primitive()).map(JsonElement::get_as_string),
        _ => None,
    }
}
