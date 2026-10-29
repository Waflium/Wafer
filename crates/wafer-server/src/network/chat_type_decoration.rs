use serde_json::Value;

use crate::network::chat::component_serialization::decode_style_fields;
use crate::network::chat::{Component, Style};
use crate::network::chat_type::Bound;
use crate::world::item::component::json_to_nbt;

#[derive(Clone, Copy, Debug)]
pub enum Parameter {
    Sender,
    Target,
    Content,
}

impl Parameter {
    fn by_name(name: &str) -> Option<Parameter> {
        match name {
            "sender" => Some(Parameter::Sender),
            "target" => Some(Parameter::Target),
            "content" => Some(Parameter::Content),
            _ => None,
        }
    }

    pub fn select(self, content: &Component, chat_type: &Bound) -> Component {
        match self {
            Parameter::Sender => chat_type.name.clone(),
            Parameter::Target => chat_type.target_name.clone().unwrap_or_else(Component::empty),
            Parameter::Content => content.clone(),
        }
    }
}

#[derive(Debug)]
pub struct ChatTypeDecoration {
    translation_key: String,
    parameters: Vec<Parameter>,
    style: Style,
}

impl ChatTypeDecoration {
    pub fn parse(json: &Value) -> ChatTypeDecoration {
        let style = match json_to_nbt(&json["style"]) {
            wafer_protocol::nbt::Nbt::Compound(fields) => decode_style_fields(&fields).unwrap_or_default(),
            _ => Style::default(),
        };
        ChatTypeDecoration {
            translation_key: json["translation_key"].as_str().unwrap_or_default().to_owned(),
            parameters: json["parameters"].as_array().into_iter().flatten().filter_map(|parameter| parameter.as_str().and_then(Parameter::by_name)).collect(),
            style,
        }
    }

    pub fn decorate(&self, content: Component, chat_type: &Bound) -> Component {
        let parameters = self.parameters.iter().map(|parameter| parameter.select(&content, chat_type).into()).collect();
        let mut component = Component::translatable_with(&self.translation_key, parameters);
        component.style = self.style.apply_to(&component.style);
        component
    }
}
