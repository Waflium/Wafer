#[cfg(test)]
mod component_serialization_tests;
pub mod component_serialization;
pub mod component_utils;
pub mod last_seen_messages;
pub mod last_seen_messages_validator;
pub mod message_signature;
pub mod message_signature_cache;
pub mod nbt_contents;
pub mod numbers;
pub mod player_chat_message;
pub mod remote_chat_session;
pub mod resolution_context;
pub mod signable_command;
#[cfg(test)]
mod signed_chat_tests;
pub mod signed_message_body;
pub mod signed_message_chain;
pub mod signed_message_link;

use std::collections::HashMap;
use std::sync::LazyLock;

use wafer_protocol::nbt::Nbt;

use crate::commands::arguments::selector::entity_selector::EntitySelector;

#[derive(Clone, Copy, Debug)]
pub struct TextColor {
    pub value: u32,
    pub name: Option<&'static str>,
}

impl PartialEq for TextColor {
    fn eq(&self, other: &TextColor) -> bool {
        self.value == other.value
    }
}

const NAMED_COLORS: [(&str, u32); 16] = [
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

impl TextColor {
    pub fn from_rgb(rgb: u32) -> TextColor {
        TextColor { value: rgb & 0xFFFFFF, name: None }
    }

    pub fn named(index: usize) -> TextColor {
        let (name, value) = NAMED_COLORS[index];
        TextColor { value, name: Some(name) }
    }

    pub fn serialize(&self) -> String {
        match self.name {
            Some(name) => name.to_string(),
            None => format!("#{:06X}", self.value),
        }
    }

    pub fn parse_color(color: &str) -> Result<TextColor, String> {
        if let Some(hex) = color.strip_prefix('#') {
            let digits = hex.strip_prefix(['+', '-']).unwrap_or(hex);
            if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Err(format!("Invalid color value: {color}"));
            }
            return match i32::from_str_radix(hex, 16) {
                Ok(value) if (0..=0xFFFFFF).contains(&value) => Ok(TextColor::from_rgb(value as u32)),
                Ok(_) => Err(format!("Color value out of range: {color}")),
                Err(_) => Err(format!("Invalid color value: {color}")),
            };
        }
        NAMED_COLORS.iter().find(|c| c.0 == color).map(|&(name, value)| TextColor { value, name: Some(name) }).ok_or_else(|| format!("Invalid color name: {color}"))
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ChatFormatting {
    Black,
    DarkBlue,
    DarkGreen,
    DarkAqua,
    DarkRed,
    DarkPurple,
    Gold,
    Gray,
    DarkGray,
    Blue,
    Green,
    Aqua,
    Red,
    LightPurple,
    Yellow,
    White,
}

impl ChatFormatting {
    pub fn color(self) -> TextColor {
        TextColor::named(self as usize)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum ClickEvent {
    SuggestCommand(String),
    RunCommand(String),
    CopyToClipboard(String),
    OpenUrl(String),
    ChangePage(i32),
    Other { action: &'static str, fields: Vec<(String, Nbt)> },
}

#[derive(Clone, Debug, PartialEq)]
pub enum HoverEvent {
    ShowText(Box<Component>),
    ShowEntity { id: &'static str, uuid: u128, name: Option<Box<Component>> },
    ShowItem { id: &'static str, count: i32, components: Vec<(String, Nbt)> },
}

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Style {
    pub color: Option<TextColor>,
    pub shadow_color: Option<i32>,
    pub bold: Option<bool>,
    pub italic: Option<bool>,
    pub underlined: Option<bool>,
    pub strikethrough: Option<bool>,
    pub obfuscated: Option<bool>,
    pub click_event: Option<ClickEvent>,
    pub hover_event: Option<HoverEvent>,
    pub insertion: Option<String>,
    pub font: Option<String>,
}

impl Style {
    pub fn is_empty(&self) -> bool {
        *self == Style::default()
    }

    pub fn apply_to(&self, other: &Style) -> Style {
        Style {
            color: self.color.or(other.color),
            shadow_color: self.shadow_color.or(other.shadow_color),
            bold: self.bold.or(other.bold),
            italic: self.italic.or(other.italic),
            underlined: self.underlined.or(other.underlined),
            strikethrough: self.strikethrough.or(other.strikethrough),
            obfuscated: self.obfuscated.or(other.obfuscated),
            click_event: self.click_event.clone().or_else(|| other.click_event.clone()),
            hover_event: self.hover_event.clone().or_else(|| other.hover_event.clone()),
            insertion: self.insertion.clone().or_else(|| other.insertion.clone()),
            font: self.font.clone().or_else(|| other.font.clone()),
        }
    }

    pub fn write_fields(&self, fields: &mut Vec<(String, Nbt)>) {
        let s = self;
        if let Some(color) = s.color {
            fields.push(("color".into(), Nbt::String(color.serialize())));
        }
        if let Some(shadow) = s.shadow_color {
            fields.push(("shadow_color".into(), Nbt::Int(shadow)));
        }
        for (name, value) in [("bold", s.bold), ("italic", s.italic), ("underlined", s.underlined), ("strikethrough", s.strikethrough), ("obfuscated", s.obfuscated)] {
            if let Some(v) = value {
                fields.push((name.into(), Nbt::Byte(v as i8)));
            }
        }
        if let Some(click) = &s.click_event {
            let event = match click {
                ClickEvent::SuggestCommand(c) => vec![("action".into(), Nbt::String("suggest_command".into())), ("command".into(), Nbt::String(c.clone()))],
                ClickEvent::RunCommand(c) => vec![("action".into(), Nbt::String("run_command".into())), ("command".into(), Nbt::String(c.clone()))],
                ClickEvent::CopyToClipboard(v) => vec![("action".into(), Nbt::String("copy_to_clipboard".into())), ("value".into(), Nbt::String(v.clone()))],
                ClickEvent::OpenUrl(url) => vec![("action".into(), Nbt::String("open_url".into())), ("url".into(), Nbt::String(url.clone()))],
                ClickEvent::ChangePage(page) => vec![("action".into(), Nbt::String("change_page".into())), ("page".into(), Nbt::Int(*page))],
                ClickEvent::Other { action, fields } => {
                    let mut f = vec![("action".into(), Nbt::String((*action).into()))];
                    f.extend(fields.iter().cloned());
                    f
                }
            };
            fields.push(("click_event".into(), Nbt::Compound(event.into())));
        }
        if let Some(hover) = &s.hover_event {
            let event = match hover {
                HoverEvent::ShowText(text) => Nbt::Compound(vec![("action".into(), Nbt::String("show_text".into())), ("value".into(), text.to_nbt())].into()),
                HoverEvent::ShowEntity { id, uuid, name } => {
                    let mut f = vec![("action".into(), Nbt::String("show_entity".into())), ("id".into(), Nbt::String((*id).into())), ("uuid".into(), uuid_ints(*uuid))];
                    if let Some(name) = name {
                        f.insert(0, ("name".into(), name.to_nbt()));
                    }
                    Nbt::Compound(f.into())
                }
                HoverEvent::ShowItem { id, count, components } => {
                    let mut f = vec![("action".into(), Nbt::String("show_item".into())), ("id".into(), Nbt::String((*id).into()))];
                    if *count != 1 {
                        f.push(("count".into(), Nbt::Int(*count)));
                    }
                    if !components.is_empty() {
                        f.push(("components".into(), Nbt::Compound(components.clone().into())));
                    }
                    Nbt::Compound(f.into())
                }
            };
            fields.push(("hover_event".into(), event));
        }
        if let Some(insertion) = &s.insertion {
            fields.push(("insertion".into(), Nbt::String(insertion.clone())));
        }
        if let Some(font) = &s.font {
            fields.push(("font".into(), Nbt::String(font.clone())));
        }
    }

    pub fn to_nbt(&self) -> Nbt {
        let mut fields = Vec::new();
        self.write_fields(&mut fields);
        Nbt::Compound(fields.into())
    }

    pub fn java_to_string(&self) -> String {
        let mut result = String::from("{");
        let mut first = true;
        let mut separate = |result: &mut String| {
            if !first {
                result.push(',');
            }
            first = false;
        };
        if let Some(color) = &self.color {
            separate(&mut result);
            result.push_str(&format!("color={}", color.serialize()));
        }
        if let Some(shadow_color) = self.shadow_color {
            separate(&mut result);
            result.push_str(&format!("shadowColor={shadow_color}"));
        }
        for (name, value) in [("bold", self.bold), ("italic", self.italic), ("underlined", self.underlined), ("strikethrough", self.strikethrough), ("obfuscated", self.obfuscated)] {
            if let Some(value) = value {
                separate(&mut result);
                if !value {
                    result.push('!');
                }
                result.push_str(name);
            }
        }
        if let Some(click_event) = &self.click_event {
            separate(&mut result);
            result.push_str("clickEvent=");
            result.push_str(&match click_event {
                ClickEvent::SuggestCommand(command) => format!("SuggestCommand[command={command}]"),
                ClickEvent::RunCommand(command) => format!("RunCommand[command={command}]"),
                ClickEvent::CopyToClipboard(value) => format!("CopyToClipboard[value={value}]"),
                ClickEvent::OpenUrl(uri) => format!("OpenUrl[uri={uri}]"),
                ClickEvent::ChangePage(page) => format!("ChangePage[page={page}]"),
                ClickEvent::Other { action, .. } => format!("{action}[]"),
            });
        }
        if let Some(hover_event) = &self.hover_event {
            separate(&mut result);
            result.push_str("hoverEvent=");
            result.push_str(&match hover_event {
                HoverEvent::ShowText(text) => format!("ShowText[value={}]", text.java_to_string()),
                HoverEvent::ShowEntity { .. } => "ShowEntity[]".to_string(),
                HoverEvent::ShowItem { .. } => "ShowItem[]".to_string(),
            });
        }
        if let Some(insertion) = &self.insertion {
            separate(&mut result);
            result.push_str(&format!("insertion={insertion}"));
        }
        if let Some(font) = &self.font {
            separate(&mut result);
            result.push_str(&format!("font=Resource[id={font}]"));
        }
        result.push('}');
        result
    }
}

impl Arg {
    fn java_to_string(&self) -> String {
        match self {
            Arg::Component(component) => component.java_to_string(),
            other => other.plain(),
        }
    }
}

impl Contents {
    fn java_to_string(&self) -> String {
        match self {
            Contents::Literal(text) if text.is_empty() => "empty".to_string(),
            Contents::Literal(text) => format!("literal{{{text}}}"),
            Contents::Translatable { key, fallback, args } => {
                let fallback = fallback.as_ref().map_or_else(String::new, |fallback| format!(", fallback='{fallback}'"));
                let args: Vec<String> = args.iter().map(Arg::java_to_string).collect();
                format!("translation{{key='{key}'{fallback}, args=[{}]}}", args.join(", "))
            }
            Contents::Keybind(name) => format!("keybind{{{name}}}"),
            Contents::Score { name, selector, objective } => {
                let side = if selector.is_some() { "Left" } else { "Right" };
                format!("score{{name='{side}[{name}]', objective='{objective}'}}")
            }
            Contents::Selector { pattern, .. } => format!("pattern{{{pattern}}}"),
            Contents::Other(fields) => {
                let field = |name: &str| fields.iter().find(|(key, _)| key == name).map(|(_, value)| value);
                let Some(path) = field("nbt").and_then(Nbt::as_str) else { return String::new() };
                let flag = |name: &str| field(name).and_then(Nbt::as_i64).is_some_and(|value| value != 0);
                let separator = field("separator").map_or_else(|| "Optional.empty".to_string(), |separator| format!("Optional[{}]", Component::from_nbt(separator).java_to_string()));
                let data_source = if let Some(selector) = field("entity").and_then(Nbt::as_str) {
                    format!("EntityDataSource[selector={selector}]")
                } else if let Some(coordinates) = field("block").and_then(Nbt::as_str) {
                    format!("BlockDataSource[coordinates={coordinates}]")
                } else {
                    format!("storage={}", field("storage").and_then(Nbt::as_str).unwrap_or_default())
                };
                format!("NbtContents[nbtPath={path}, interpreting={}, plain={}, separator={separator}, dataSource={data_source}]", flag("interpret"), flag("plain"))
            }
            Contents::NbtContents(contents) => {
                let separator = contents.separator.as_ref().map_or_else(|| "Optional.empty".to_string(), |separator| format!("Optional[{}]", separator.java_to_string()));
                let data_source = match &contents.data_source {
                    nbt_contents::DataSource::Entity(selector) => format!("EntityDataSource[selector={}]", selector.source()),
                    nbt_contents::DataSource::Block(coordinates) => format!("BlockDataSource[coordinates={}]", coordinates.source()),
                    nbt_contents::DataSource::Storage(id) => format!("storage={id}"),
                };
                format!(
                    "NbtContents[nbtPath={}, interpreting={}, plain={}, separator={separator}, dataSource={data_source}]",
                    contents.nbt_path.source(),
                    contents.interpreting,
                    contents.plain
                )
            }
            Contents::Nbt(_) => String::new(),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Component(Component),
    Byte(i8),
    Short(i16),
    Int(i32),
    Long(i64),
    Float(f32),
    Double(f64),
    String(String),
}

impl From<Component> for Arg {
    fn from(c: Component) -> Arg {
        Arg::Component(c)
    }
}

impl From<i32> for Arg {
    fn from(v: i32) -> Arg {
        Arg::Int(v)
    }
}

impl From<i64> for Arg {
    fn from(v: i64) -> Arg {
        Arg::Long(v)
    }
}

impl From<f32> for Arg {
    fn from(v: f32) -> Arg {
        Arg::Float(v)
    }
}

impl From<f64> for Arg {
    fn from(v: f64) -> Arg {
        Arg::Double(v)
    }
}

impl From<&str> for Arg {
    fn from(v: &str) -> Arg {
        Arg::String(v.into())
    }
}

impl From<String> for Arg {
    fn from(v: String) -> Arg {
        Arg::String(v)
    }
}

impl Arg {
    fn plain(&self) -> String {
        match self {
            Arg::Component(c) => c.get_string(),
            Arg::Byte(v) => v.to_string(),
            Arg::Short(v) => v.to_string(),
            Arg::Int(v) => v.to_string(),
            Arg::Long(v) => v.to_string(),
            Arg::Float(v) => crate::nbt::java_float_to_string(*v),
            Arg::Double(v) => crate::nbt::java_double_to_string(*v),
            Arg::String(s) => s.clone(),
        }
    }

    fn to_nbt(&self) -> Nbt {
        match self {
            Arg::Component(c) => c.to_nbt(),
            Arg::Byte(v) => Nbt::Byte(*v),
            Arg::Short(v) => Nbt::Short(*v),
            Arg::Int(v) => Nbt::Int(*v),
            Arg::Long(v) => Nbt::Long(*v),
            Arg::Float(v) => Nbt::Float(*v),
            Arg::Double(v) => Nbt::Double(*v),
            Arg::String(s) => Nbt::String(s.clone()),
        }
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum Contents {
    Literal(String),
    Translatable { key: String, fallback: Option<String>, args: Vec<Arg> },
    Keybind(String),
    Score { name: String, selector: Option<Box<EntitySelector>>, objective: String },
    Selector { pattern: String, selector: Box<EntitySelector>, separator: Option<Box<Component>> },
    NbtContents(Box<nbt_contents::NbtContents>),
    Other(Vec<(String, Nbt)>),
    Nbt(Nbt),
}

#[derive(Clone, Debug, PartialEq)]
pub struct Component {
    pub contents: Contents,
    pub style: Style,
    pub siblings: Vec<Component>,
}

impl Component {
    pub fn literal(text: impl Into<String>) -> Component {
        Component { contents: Contents::Literal(text.into()), style: Style::default(), siblings: Vec::new() }
    }

    pub fn empty() -> Component {
        Component::literal("")
    }

    pub fn translatable(key: &str) -> Component {
        Component::translatable_with(key, Vec::new())
    }

    pub fn translatable_with(key: &str, args: Vec<Arg>) -> Component {
        Component { contents: Contents::Translatable { key: key.into(), fallback: None, args }, style: Style::default(), siblings: Vec::new() }
    }

    pub fn translatable_with_fallback(key: &str, fallback: &str, args: Vec<Arg>) -> Component {
        Component { contents: Contents::Translatable { key: key.into(), fallback: Some(fallback.into()), args }, style: Style::default(), siblings: Vec::new() }
    }

    pub fn append(mut self, sibling: Component) -> Component {
        self.siblings.push(sibling);
        self
    }

    pub fn with_color(mut self, color: ChatFormatting) -> Component {
        self.style.color = Some(color.color());
        self
    }

    pub fn with_text_color(mut self, color: TextColor) -> Component {
        self.style.color = Some(color);
        self
    }

    pub fn with_style(mut self, style: &Style) -> Component {
        self.style = style.apply_to(&self.style);
        self
    }

    pub fn italic(mut self) -> Component {
        self.style.italic = Some(true);
        self
    }

    pub fn underlined(mut self) -> Component {
        self.style.underlined = Some(true);
        self
    }

    pub fn with_click(mut self, event: ClickEvent) -> Component {
        self.style.click_event = Some(event);
        self
    }

    pub fn with_hover(mut self, event: HoverEvent) -> Component {
        self.style.hover_event = Some(event);
        self
    }

    pub fn with_insertion(mut self, insertion: &str) -> Component {
        self.style.insertion = Some(insertion.into());
        self
    }

    pub fn from_nbt(tag: &Nbt) -> Component {
        component_serialization::decode(tag).unwrap_or_else(|_| Component { contents: Contents::Nbt(tag.clone()), style: Style::default(), siblings: Vec::new() })
    }

    pub fn to_nbt(&self) -> Nbt {
        if let Contents::Literal(text) = &self.contents
            && self.style.is_empty()
            && self.siblings.is_empty()
        {
            return Nbt::String(text.clone());
        }
        if let Contents::Nbt(tag) = &self.contents
            && self.style.is_empty()
            && self.siblings.is_empty()
        {
            return tag.clone();
        }
        let mut fields: Vec<(String, Nbt)> = Vec::new();
        match &self.contents {
            Contents::Literal(text) => fields.push(("text".into(), Nbt::String(text.clone()))),
            Contents::Nbt(tag) => {
                fields.push(("text".into(), Nbt::String(String::new())));
                let mut siblings = vec![tag.clone()];
                siblings.extend(self.siblings.iter().map(Component::to_nbt));
                fields.push(("extra".into(), Nbt::List(siblings)));
            }
            Contents::Translatable { key, fallback, args } => {
                fields.push(("translate".into(), Nbt::String(key.clone())));
                if let Some(fallback) = fallback {
                    fields.push(("fallback".into(), Nbt::String(fallback.clone())));
                }
                if !args.is_empty() {
                    fields.push(("with".into(), Nbt::List(args.iter().map(Arg::to_nbt).collect())));
                }
            }
            Contents::Keybind(keybind) => fields.push(("keybind".into(), Nbt::String(keybind.clone()))),
            Contents::Score { name, objective, .. } => fields.push((
                "score".into(),
                Nbt::Compound(vec![("name".into(), Nbt::String(name.clone())), ("objective".into(), Nbt::String(objective.clone()))].into()),
            )),
            Contents::Selector { pattern, separator, .. } => {
                fields.push(("selector".into(), Nbt::String(pattern.clone())));
                if let Some(separator) = separator {
                    fields.push(("separator".into(), separator.to_nbt()));
                }
            }
            Contents::NbtContents(contents) => {
                fields.push(("nbt".into(), Nbt::String(contents.nbt_path.source().into())));
                if contents.interpreting {
                    fields.push(("interpret".into(), Nbt::Byte(1)));
                }
                if contents.plain {
                    fields.push(("plain".into(), Nbt::Byte(1)));
                }
                if let Some(separator) = &contents.separator {
                    fields.push(("separator".into(), separator.to_nbt()));
                }
                let (key, value) = contents.data_source.to_field();
                fields.push((key.into(), value));
            }
            Contents::Other(raw) => fields.extend(raw.iter().cloned()),
        }
        if !self.siblings.is_empty() && !matches!(self.contents, Contents::Nbt(_)) {
            fields.push(("extra".into(), Nbt::List(self.siblings.iter().map(Component::to_nbt).collect())));
        }
        self.style.write_fields(&mut fields);
        Nbt::Compound(fields.into())
    }

    pub fn to_json(&self) -> String {
        self.to_json_value().to_string()
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        nbt_to_json(&self.to_nbt())
    }

    pub fn write(&self, w: &mut wafer_protocol::Writer) {
        w.layer(wafer_protocol::stream_codec::FROM_CODEC, |w| self.to_nbt().write(w));
    }

    pub fn write_trusted_context_free(&self, w: &mut wafer_protocol::Writer) {
        w.layer(wafer_protocol::stream_codec::MAP, |w| w.layer(wafer_protocol::stream_codec::COMPOUND_TAG_CODEC, |w| self.to_nbt().write(w)));
    }

    pub fn get_string_with_limit(&self, limit: usize) -> String {
        let mut string = self.get_string();
        let mut length = 0;
        if let Some((end, _)) = string.char_indices().find(|&(_, c)| {
            length += c.len_utf16();
            length > limit
        }) {
            string.truncate(end);
        }
        string
    }

    pub fn java_to_string(&self) -> String {
        let mut result = self.contents.java_to_string();
        let has_style = !self.style.is_empty();
        let has_siblings = !self.siblings.is_empty();
        if has_style || has_siblings {
            result.push('[');
            if has_style {
                result.push_str("style=");
                result.push_str(&self.style.java_to_string());
            }
            if has_style && has_siblings {
                result.push_str(", ");
            }
            if has_siblings {
                let siblings: Vec<String> = self.siblings.iter().map(Component::java_to_string).collect();
                result.push_str(&format!("siblings=[{}]", siblings.join(", ")));
            }
            result.push(']');
        }
        result
    }

    pub fn get_string(&self) -> String {
        let mut out = String::new();
        self.append_string(&mut out);
        out
    }

    fn append_string(&self, out: &mut String) {
        match &self.contents {
            Contents::Literal(text) => out.push_str(text),
            Contents::Nbt(tag) => append_nbt_string(tag, out),
            Contents::Translatable { key, fallback, args } => {
                let format = language().get(key.as_str()).map(String::as_str).or(fallback.as_deref()).unwrap_or(key);
                match decompose(format, args) {
                    Some(text) => out.push_str(&text),
                    None => out.push_str(format),
                }
            }
            Contents::Keybind(keybind) => out.push_str(keybind),
            Contents::Selector { pattern, .. } => out.push_str(pattern),
            Contents::Other(fields) => {
                let field = |name: &str| fields.iter().find(|(k, _)| k == name).map(|(_, v)| v);
                if let Some(fallback) = field("fallback") {
                    Component::from_nbt(fallback).append_string(out);
                } else if let Some(player) = field("player") {
                    match player.as_str().or_else(|| player.get("name").and_then(Nbt::as_str)) {
                        Some(name) => out.push_str(&format!("[{name} head]")),
                        None => out.push_str("[unknown player head]"),
                    }
                }
            }
            Contents::Score { .. } | Contents::NbtContents(_) => {}
        }
        for sibling in &self.siblings {
            sibling.append_string(out);
        }
    }
}

fn append_nbt_string(tag: &Nbt, out: &mut String) {
    match tag {
        Nbt::String(s) => out.push_str(s),
        Nbt::Compound(_) => {
            if let Some(text) = tag.get("text").and_then(Nbt::as_str) {
                out.push_str(text);
            } else if let Some(key) = tag.get("translate").and_then(Nbt::as_str) {
                out.push_str(language().get(key).map_or(key, String::as_str));
            }
            for sibling in tag.get("extra").map(Nbt::as_list).unwrap_or(&[]) {
                append_nbt_string(sibling, out);
            }
        }
        _ => {}
    }
}

fn apply_deprecated_translations(translations: &mut HashMap<String, String>) {
    let deprecated_info: serde_json::Value = serde_json::from_str(wafer_data::LANGUAGE_DEPRECATED).expect("deprecated.json");
    for key in deprecated_info["removed"].as_array().into_iter().flatten().filter_map(serde_json::Value::as_str) {
        translations.remove(key);
    }
    for (from_key, to_key) in deprecated_info["renamed"].as_object().into_iter().flatten() {
        let Some(to_key) = to_key.as_str() else { continue };
        match translations.remove(from_key) {
            Some(value) => {
                translations.insert(to_key.to_string(), value);
            }
            None => {
                warn!("Missing translation key for rename: {from_key}");
                translations.remove(to_key);
            }
        }
    }
}

fn language() -> &'static HashMap<String, String> {
    static LANGUAGE: LazyLock<HashMap<String, String>> = LazyLock::new(|| {
        let mut translations = serde_json::from_str(wafer_data::LANGUAGE_EN_US).expect("en_us.json");
        apply_deprecated_translations(&mut translations);
        translations
    });
    &LANGUAGE
}

fn decompose(template: &str, args: &[Arg]) -> Option<String> {
    let mut out = String::new();
    let mut next = 0usize;
    let mut chars = template.char_indices().peekable();
    while let Some((_, c)) = chars.next() {
        if c != '%' {
            out.push(c);
            continue;
        }
        let mut digits = String::new();
        while let Some(&(_, d)) = chars.peek() {
            if d.is_ascii_digit() {
                digits.push(d);
                chars.next();
            } else {
                break;
            }
        }
        let positional = if !digits.is_empty() {
            match chars.next() {
                Some((_, '$')) => Some(digits.parse::<usize>().ok()?),
                _ => return None,
            }
        } else {
            None
        };
        match chars.next() {
            Some((_, '%')) if positional.is_none() => out.push('%'),
            Some((_, 's')) => {
                let index = match positional {
                    Some(p) => p.checked_sub(1)?,
                    None => {
                        next += 1;
                        next - 1
                    }
                };
                out.push_str(&args.get(index)?.plain());
            }
            _ => return None,
        }
    }
    Some(out)
}

fn nbt_to_json(nbt: &Nbt) -> serde_json::Value {
    use serde_json::Value;
    match nbt {
        Nbt::Byte(b) => Value::Bool(*b != 0),
        Nbt::Int(i) => Value::from(*i),
        Nbt::Long(l) => Value::from(*l),
        Nbt::Float(f) => Value::from(*f),
        Nbt::Double(d) => Value::from(*d),
        Nbt::String(s) => Value::String(s.clone()),
        Nbt::IntArray(a) => Value::Array(a.iter().map(|&i| Value::from(i)).collect()),
        Nbt::List(items) => Value::Array(
            items
                .iter()
                .map(|item| match item {
                    Nbt::Compound(f) if f.len() == 1 && f[0].0.is_empty() => nbt_to_json(&f[0].1),
                    other => nbt_to_json(other),
                })
                .collect(),
        ),
        Nbt::Compound(fields) => Value::Object(fields.iter().map(|(k, v)| (k.clone(), nbt_to_json(v))).collect()),
        other => unreachable!("components have no {other:?}"),
    }
}

fn uuid_ints(uuid: u128) -> Nbt {
    Nbt::IntArray((0..4).map(|i| (uuid >> (96 - 32 * i)) as u32 as i32).collect())
}

pub fn decorate_display_name(component: Component, name: &str, uuid: u128) -> Component {
    component
        .with_click(ClickEvent::SuggestCommand(format!("/tell {name} ")))
        .with_hover(HoverEvent::ShowEntity { id: "minecraft:player", uuid, name: Some(Box::new(Component::literal(name))) })
        .with_insertion(name)
}

pub fn player_joined_or_left(display_name: Component, joined: bool) -> Component {
    let key = if joined { "multiplayer.player.joined" } else { "multiplayer.player.left" };
    Component::translatable_with(key, vec![display_name.into()]).with_color(ChatFormatting::Yellow)
}

pub fn translate(key: &str) -> Component {
    Component::translatable(key)
}

#[cfg(test)]
mod tests {
    use super::*;
    use wafer_protocol::Writer;

    #[test]
    fn join_message_matches_vanilla() {
        let uuid = 0xfaa5dca3_c3d4354b_ae1bdde9_e5a14b3b;
        let mut w = Writer::new();
        player_joined_or_left(decorate_display_name(Component::literal("Bob"), "Bob", uuid), true).write(&mut w);
        let hex: String = w.as_slice().iter().map(|b| format!("{b:02x}")).collect();
        let vanilla = concat!(
            "0a090004776974680a000000010a000b636c69636b5f6576656e74080006616374696f6e000f737567676573745f636f",
            "6d6d616e64080007636f6d6d616e64000a2f74656c6c20426f622000080009696e73657274696f6e0003426f6208000474",
            "6578740003426f620a000b686f7665725f6576656e740800046e616d650003426f62080006616374696f6e000b73686f77",
            "5f656e74697479080002696400106d696e6563726166743a706c617965720b00047575696400000004faa5dca3c3d4354b",
            "ae1bdde9e5a14b3b0000080005636f6c6f72000679656c6c6f770800097472616e736c61746500196d756c7469706c6179",
            "65722e706c617965722e6a6f696e656400",
        );
        assert_eq!(hex, vanilla);
    }

    #[test]
    fn renders_translations() {
        let c = Component::translatable_with("commands.setblock.success", vec![1.into(), (-60).into(), 2.into()]);
        assert_eq!(c.get_string(), "Changed the block at 1, -60, 2");
        assert_eq!(player_joined_or_left(decorate_display_name(Component::literal("Bob"), "Bob", 1), true).get_string(), "Bob joined the game");
    }
}
