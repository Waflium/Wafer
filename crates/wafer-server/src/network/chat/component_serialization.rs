use wafer_data::registry;
use wafer_protocol::nbt::Nbt;

use super::nbt_contents::{DataSource, NbtContents};
use super::{Arg, ClickEvent, Component, Contents, HoverEvent, Style, TextColor};
use crate::commands::arguments::coordinates::{Coordinates, parse_block_pos};
use crate::commands::arguments::nbt_path_argument::{self, NbtPath};
use crate::commands::arguments::parse_identifier;
use crate::commands::arguments::selector::entity_selector::EntitySelector;
use crate::commands::arguments::selector::entity_selector_parser::EntitySelectorParser;
use crate::util::compilable_string::{CompilableString, command_parser_helper};
use crate::world::item::component::{DataComponentPatch, custom};
use crate::world::level::storage::tag_value_input;

struct DataResult<T> {
    value: Option<T>,
    error: Option<String>,
}

impl<T> DataResult<T> {
    fn success(value: T) -> DataResult<T> {
        DataResult { value: Some(value), error: None }
    }

    fn error(message: impl Into<String>) -> DataResult<T> {
        DataResult { value: None, error: Some(message.into()) }
    }

    fn from_result(result: std::result::Result<T, String>) -> DataResult<T> {
        match result {
            Ok(value) => DataResult::success(value),
            Err(message) => DataResult::error(message),
        }
    }

    fn is_success(&self) -> bool {
        self.error.is_none()
    }

    fn map<U>(self, f: impl FnOnce(T) -> U) -> DataResult<U> {
        DataResult { value: self.value.map(f), error: self.error }
    }

    fn flat_map<U>(self, f: impl FnOnce(T) -> DataResult<U>) -> DataResult<U> {
        let Some(value) = self.value else { return DataResult { value: None, error: self.error } };
        let second = f(value);
        let error = match (self.error, second.error) {
            (Some(first), Some(second)) => Some(format!("{first}; {second}")),
            (first, second) => first.or(second),
        };
        DataResult { value: second.value, error }
    }

    fn into_result(self) -> std::result::Result<T, String> {
        match self.error {
            None => Ok(self.value.expect("a successful value")),
            Some(message) => Err(message),
        }
    }
}

fn record_error(errors: &[&Option<String>]) -> Option<String> {
    let messages: Vec<&str> = errors.iter().rev().filter_map(|error| error.as_deref()).collect();
    (!messages.is_empty()).then(|| messages.join("; "))
}

fn either<T>(first: DataResult<T>, second: impl FnOnce() -> DataResult<T>) -> DataResult<T> {
    if first.is_success() {
        return first;
    }
    let second = second();
    if second.is_success() {
        return second;
    }
    if first.value.is_some() {
        return first;
    }
    if second.value.is_some() {
        return second;
    }
    DataResult::error(format!("Failed to parse either. First: {}; Second: {}", first.error.expect("an error"), second.error.expect("an error")))
}

fn optional<T>(tag: Option<&Nbt>, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<Option<T>> {
    match tag {
        None => DataResult::success(None),
        Some(tag) => {
            let result = decode(tag);
            DataResult { value: Some(result.value), error: result.error }
        }
    }
}

fn lenient_optional<T>(tag: Option<&Nbt>, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> Option<T> {
    tag.map(decode).filter(DataResult::is_success).and_then(|result| result.value)
}

fn json() -> bool {
    crate::util::codec::is_json_value()
}

fn not_a_map(tag: &Nbt) -> String {
    format!("{}: {}", if json() { "Not a JSON object" } else { "Not a map" }, snbt(tag))
}

fn list<T>(tag: &Nbt,decode: impl Fn(&Nbt) -> DataResult<T>) -> DataResult<Vec<T>> {
    let Nbt::List(items) = tag else { return DataResult::error(format!("{}: {}", if json() { "Not a json array" } else { "Not a list" }, snbt(tag))) };
    let mut elements = Vec::new();
    let mut errors = Vec::new();
    for item in items {
        let result = decode(item);
        if let Some(error) = result.error {
            errors.push(error);
        }
        if let Some(value) = result.value {
            elements.push(value);
        }
    }
    errors.reverse();
    DataResult { value: Some(elements), error: (!errors.is_empty()).then(|| errors.join("; ")) }
}

fn non_empty_list<T>(tag: &Nbt, decode: impl Fn(&Nbt) -> DataResult<T>) -> DataResult<Vec<T>> {
    list(tag, decode).flat_map(|elements| if elements.is_empty() { DataResult::error("List must have contents") } else { DataResult::success(elements) })
}

const STYLE_KEYS: [&str; 11] = ["color", "shadow_color", "bold", "italic", "underlined", "strikethrough", "obfuscated", "click_event", "hover_event", "insertion", "font"];

fn snbt(tag: &Nbt) -> String {
    crate::nbt::to_string(tag)
}

fn get<'a>(fields: &'a [(String, Nbt)], key: &str) -> Option<&'a Nbt> {
    fields.iter().find(|(k, v)| k == key && !matches!(v, Nbt::End)).map(|(_, v)| v)
}

fn no_key(key: &str, fields: &[(String, Nbt)]) -> String {
    format!("No key {key} in MapLike[{}]", snbt(&Nbt::Compound(fields.to_vec())))
}

fn required<T>(fields: &[(String, Nbt)], key: &str, decode: impl FnOnce(&Nbt) -> DataResult<T>) -> DataResult<T> {
    match get(fields, key) {
        Some(tag) => decode(tag),
        None => DataResult::error(no_key(key, fields)),
    }
}

fn string(tag: &Nbt) -> DataResult<String> {
    match tag {
        Nbt::String(s) => DataResult::success(s.clone()),
        _ if json() => DataResult::error(format!("Not a string: {}", snbt(tag))),
        _ => DataResult::error("Not a string"),
    }
}

fn boolean(tag: &Nbt) -> DataResult<bool> {
    if json() {
        return match tag {
            Nbt::Byte(value) => DataResult::success(*value != 0),
            _ => DataResult::error(format!("Not a boolean: {}", snbt(tag))),
        };
    }
    match tag.as_f64() {
        Some(value) => DataResult::success(value != 0.0),
        None => DataResult::error("Not a number"),
    }
}

fn not_a_number(tag: &Nbt) -> String {
    if json() { format!("Not a number: {}", snbt(tag)) } else { "Not a number".to_string() }
}

fn number(tag: &Nbt) -> Option<f64> {
    match tag {
        Nbt::Byte(_) if json() => None,
        Nbt::Byte(v) => Some(*v as f64),
        Nbt::Short(v) => Some(*v as f64),
        Nbt::Int(v) => Some(*v as f64),
        Nbt::Long(v) => Some(*v as f64),
        Nbt::Float(v) => Some(*v as f64),
        Nbt::Double(v) => Some(*v),
        _ => None,
    }
}

fn int(tag: &Nbt) -> DataResult<i32> {
    match tag {
        Nbt::Byte(_) if json() => DataResult::error(not_a_number(tag)),
        Nbt::Byte(v) => DataResult::success(*v as i32),
        Nbt::Short(v) => DataResult::success(*v as i32),
        Nbt::Int(v) => DataResult::success(*v),
        Nbt::Long(v) => DataResult::success(*v as i32),
        Nbt::Float(v) => DataResult::success(*v as i32),
        Nbt::Double(v) => DataResult::success(*v as i32),
        Nbt::LazilyParsedNumber(text) => DataResult::success(wafer_protocol::lazily_parsed_number::int_value(text)),
        _ => DataResult::error(not_a_number(tag)),
    }
}

fn float(tag: &Nbt) -> DataResult<f32> {
    match number(tag) {
        Some(value) => DataResult::success(value as f32),
        None => DataResult::error(not_a_number(tag)),
    }
}

pub fn decode(tag: &Nbt) -> std::result::Result<Component, String> {
    decode_component(tag).into_result()
}

pub fn decode_partial(tag: &Nbt) -> crate::util::codec::DecodeResult<Component> {
    let result = decode_component(tag);
    crate::util::codec::DecodeResult { value: result.value, error: result.error }
}

pub fn decode_or_partial(tag: &Nbt) -> Option<Component> {
    decode_component(tag).value
}

pub fn decode_custom_name(tag: &Nbt) -> crate::util::codec::DecodeResult<Nbt> {
    let result = decode_component(tag);
    crate::util::codec::DecodeResult { value: result.value.map(|name| name.to_nbt()), error: result.error }
}

pub fn parse_custom_name_safe(input: &Nbt, key: &str) -> Option<Nbt> {
    tag_value_input::read(input, key, decode_custom_name)
}

pub fn decode_style_fields(fields: &[(String, Nbt)]) -> std::result::Result<Style, String> {
    style_fields(fields).into_result()
}

fn decode_component(tag: &Nbt) -> DataResult<Component> {
    let literal = string(tag).map(Component::literal);
    let special = either(literal, || non_empty_list(tag, decode_component).map(create_from_list));
    either(special, || match tag {
        Nbt::Compound(fields) => decode_full(fields),
        _ => DataResult::error(not_a_map(tag)),
    })
}

fn create_from_list(list: Vec<Component>) -> Component {
    let mut list = list.into_iter();
    let mut result = list.next().expect("a non-empty list");
    result.siblings.extend(list);
    result
}

fn decode_full(fields: &[(String, Nbt)]) -> DataResult<Component> {
    let contents = decode_contents(fields);
    let extra = optional(get(fields, "extra"), |tag| non_empty_list(tag, decode_component));
    let style = style_fields(fields);
    let error = record_error(&[&contents.error, &extra.error, &style.error]);
    let value = match (contents.value, extra.value, style.value) {
        (Some(contents), Some(siblings), Some(style)) => Some(Component { contents, style, siblings: siblings.unwrap_or_default() }),
        _ => None,
    };
    DataResult { value, error }
}

const CONTENT_TYPES: [&str; 7] = ["text", "translatable", "keybind", "score", "selector", "nbt", "object"];

fn decode_contents(fields: &[(String, Nbt)]) -> DataResult<Contents> {
    create_legacy_component_matcher("type", &CONTENT_TYPES, fields, decode_content_type)
}

fn create_legacy_component_matcher<T>(type_field_name: &str, types: &[&str], fields: &[(String, Nbt)], decode_type: impl Fn(&str, &[(String, Nbt)]) -> DataResult<T>) -> DataResult<T> {
    if let Some(kind) = get(fields, type_field_name) {
        let kind = match string(kind).into_result() {
            Ok(kind) => kind,
            Err(message) => return DataResult::error(message),
        };
        if !types.contains(&kind.as_str()) {
            return DataResult::error(format!("Unknown element id: {kind}"));
        }
        return decode_type(&kind, fields);
    }
    for kind in types {
        let result = decode_type(kind, fields);
        if result.is_success() {
            return result;
        }
    }
    DataResult::error("No matching codec found")
}

fn decode_content_type(kind: &str, fields: &[(String, Nbt)]) -> DataResult<Contents> {
    match kind {
        "text" => required(fields, "text", string).map(Contents::Literal),
        "translatable" => {
            let key = required(fields, "translate", string);
            let fallback = get(fields, "fallback").and_then(|f| string(f).value);
            let args = optional(get(fields, "with"), |tag| list(tag, decode_arg));
            let error = record_error(&[&key.error, &args.error]);
            let value = match (key.value, args.value) {
                (Some(key), Some(args)) => Some(Contents::Translatable { key, fallback, args: args.unwrap_or_default() }),
                _ => None,
            };
            DataResult { value, error }
        }
        "keybind" => required(fields, "keybind", string).map(Contents::Keybind),
        "score" => required(fields, "score", |score| {
            let Nbt::Compound(score) = score else { return DataResult::error(not_a_map(score)) };
            let name = required(score, "name", |name| either(string(name), || string(name)));
            let objective = required(score, "objective", string);
            let error = record_error(&[&name.error, &objective.error]);
            let value = match (name.value, objective.value) {
                (Some(name), Some(objective)) => {
                    let selector = compile_selector(&name).ok().map(|selector| Box::new(selector.into_compiled()));
                    Some(Contents::Score { name, selector, objective })
                }
                _ => None,
            };
            DataResult { value, error }
        }),
        "selector" => {
            let selector = required(fields, "selector", |pattern| {
                string(pattern).flat_map(|pattern| DataResult::from_result(compile_selector(&pattern)).map(|selector| (pattern, selector.into_compiled())))
            });
            let separator = optional(get(fields, "separator"), decode_component);
            let error = record_error(&[&selector.error, &separator.error]);
            let value = match (selector.value, separator.value) {
                (Some((pattern, selector)), Some(separator)) => Some(Contents::Selector { pattern, selector: Box::new(selector), separator: separator.map(Box::new) }),
                _ => None,
            };
            DataResult { value, error }
        }
        "nbt" => decode_nbt_contents(fields),
        _ => decode_object_contents(fields),
    }
}

fn decode_object_contents(fields: &[(String, Nbt)]) -> DataResult<Contents> {
    let info = create_legacy_component_matcher("object", &["atlas", "player"], fields, |kind, fields| match kind {
        "atlas" => {
            let atlas = optional(get(fields, "atlas"), identifier);
            let sprite = required(fields, "sprite", identifier);
            let error = record_error(&[&atlas.error, &sprite.error]);
            DataResult { value: error.is_none().then_some(()), error }
        }
        _ => {
            let player = required(fields, "player", |player| DataResult::from_result(custom::profile().0.decode(player)));
            let hat = optional(get(fields, "hat"), boolean);
            let error = record_error(&[&player.error, &hat.error]);
            DataResult { value: error.is_none().then_some(()), error }
        }
    });
    let fallback = optional(get(fields, "fallback"), decode_component);
    let error = record_error(&[&info.error, &fallback.error]);
    let value = match (info.value, fallback.value) {
        (Some(()), Some(_)) => Some(Contents::Other(content_fields(fields))),
        _ => None,
    };
    DataResult { value, error }
}

fn identifier(tag: &Nbt) -> DataResult<String> {
    string(tag).flat_map(|id| DataResult::from_result(crate::resources::identifier::read(&id)))
}

fn content_fields(fields: &[(String, Nbt)]) -> Vec<(String, Nbt)> {
    fields.iter().filter(|(k, _)| k != "extra" && k != "type" && !STYLE_KEYS.contains(&k.as_str())).cloned().collect()
}

fn decode_nbt_contents(fields: &[(String, Nbt)]) -> DataResult<Contents> {
    let nbt_path = required(fields, "nbt", |path| string(path).flat_map(|path| DataResult::from_result(compile_nbt_path(&path))));
    let interpreting = lenient_optional(get(fields, "interpret"), boolean).unwrap_or(false);
    let plain = lenient_optional(get(fields, "plain"), boolean).unwrap_or(false);
    let separator = lenient_optional(get(fields, "separator"), decode_component).map(Box::new);
    let data_source = decode_data_source(fields);
    let error = record_error(&[&nbt_path.error, &data_source.error]);
    let value = match (nbt_path.value, data_source.value) {
        (Some(nbt_path), Some(data_source)) => Some(NbtContents { nbt_path, interpreting, plain, separator, data_source }),
        _ => None,
    };
    DataResult { value, error }
        .flat_map(|contents| if contents.interpreting && contents.plain { DataResult::error("'interpret' and 'plain' flags can't be both on") } else { DataResult::success(contents) })
        .map(|contents| Contents::NbtContents(Box::new(contents)))
}

pub fn decode_data_source_or_none(fields: &[(String, Nbt)]) -> Option<DataSource> {
    decode_data_source(fields).value
}

fn decode_data_source(fields: &[(String, Nbt)]) -> DataResult<DataSource> {
    create_legacy_component_matcher("source", &["entity", "block", "storage"], fields, |kind, fields| match kind {
        "entity" => required(fields, "entity", |selector| string(selector).flat_map(|selector| DataResult::from_result(compile_selector(&selector)).map(DataSource::Entity))),
        "block" => required(fields, "block", |pos| string(pos).flat_map(|pos| DataResult::from_result(compile_block_pos(&pos)).map(DataSource::Block))),
        _ => required(fields, "storage", |id| string(id).flat_map(|id| DataResult::from_result(crate::resources::identifier::read(&id)).map(DataSource::Storage))),
    })
}

fn compile_selector(pattern: &str) -> std::result::Result<CompilableString<EntitySelector>, String> {
    command_parser_helper(pattern, |reader| EntitySelectorParser::new(reader, true).parse(), |original, e| format!("Invalid selector component: {original}: {}", e.get_message()))
}

fn compile_block_pos(pos: &str) -> std::result::Result<CompilableString<Coordinates>, String> {
    command_parser_helper(pos, parse_block_pos, |original, e| format!("Invalid coordinates path: {original}: {}", e.get_message()))
}

fn compile_nbt_path(path: &str) -> std::result::Result<CompilableString<NbtPath>, String> {
    command_parser_helper(path, nbt_path_argument::parse, |original, e| format!("Invalid NBT path: {original}: {}", e.get_message()))
}

fn decode_arg(tag: &Nbt) -> DataResult<Arg> {
    let primitive = match tag {
        Nbt::String(s) => Arg::String(s.clone()),
        Nbt::Byte(v) => Arg::Byte(*v),
        Nbt::Short(v) => Arg::Short(*v),
        Nbt::Int(v) => Arg::Int(*v),
        Nbt::Long(v) => Arg::Long(*v),
        Nbt::Float(v) => Arg::Float(*v),
        Nbt::Double(v) => Arg::Double(*v),
        other => {
            if let Some(message) = crate::world::item::component::text::json_ops_null_pointer(other) {
                return DataResult::error(message);
            }
            return either(DataResult::error("This value needs to be parsed as component"), || decode_component(other)).map(|component| match component.contents {
                Contents::Literal(text) if component.style.is_empty() && component.siblings.is_empty() => Arg::String(text),
                _ => Arg::Component(component),
            });
        }
    };
    DataResult::success(primitive)
}

fn style_fields(fields: &[(String, Nbt)]) -> DataResult<Style> {
    let color = optional(get(fields, "color"), |color| string(color).flat_map(|color| DataResult::from_result(TextColor::parse_color(&color))));
    let shadow_color = optional(get(fields, "shadow_color"), argb_color);
    let bold = optional(get(fields, "bold"), boolean);
    let italic = optional(get(fields, "italic"), boolean);
    let underlined = optional(get(fields, "underlined"), boolean);
    let strikethrough = optional(get(fields, "strikethrough"), boolean);
    let obfuscated = optional(get(fields, "obfuscated"), boolean);
    let click_event = optional(get(fields, "click_event"), |click| DataResult::from_result(click_event(click)));
    let hover_event = optional(get(fields, "hover_event"), hover_event);
    let insertion = optional(get(fields, "insertion"), string);
    let font = optional(get(fields, "font"), |font| {
        string(font).flat_map(|font| DataResult::from_result(crate::resources::identifier::read(&font).map(|_| parse_identifier(&font).expect("a valid identifier"))))
    });
    let error = record_error(&[
        &color.error,
        &shadow_color.error,
        &bold.error,
        &italic.error,
        &underlined.error,
        &strikethrough.error,
        &obfuscated.error,
        &click_event.error,
        &hover_event.error,
        &insertion.error,
        &font.error,
    ]);
    let style = Style {
        color: color.value.flatten(),
        shadow_color: shadow_color.value.flatten(),
        bold: bold.value.flatten(),
        italic: italic.value.flatten(),
        underlined: underlined.value.flatten(),
        strikethrough: strikethrough.value.flatten(),
        obfuscated: obfuscated.value.flatten(),
        click_event: click_event.value.flatten(),
        hover_event: hover_event.value.flatten(),
        insertion: insertion.value.flatten(),
        font: font.value.flatten(),
    };
    DataResult { value: Some(style), error }
}

fn argb_color(tag: &Nbt) -> DataResult<i32> {
    either(int(tag), || {
        list(tag, float).flat_map(|channels| {
            if channels.len() != 4 {
                return DataResult::error("Input is not a list of 4 elements");
            }
            let channel = |f: f32| wafer_core::mth::floor((f * 255.0) as f64) & 0xFF;
            DataResult::success(channel(channels[3]) << 24 | channel(channels[0]) << 16 | channel(channels[1]) << 8 | channel(channels[2]))
        })
    })
}

fn click_event(tag: &Nbt) -> std::result::Result<ClickEvent, String> {
    let Nbt::Compound(fields) = tag else { return Err(not_a_map(tag)) };
    let field = |key: &str| required(fields, key, string).into_result();
    let action = field("action")?;
    Ok(match action.as_str() {
        "open_url" => ClickEvent::OpenUrl(field("url")?),
        "run_command" => ClickEvent::RunCommand(field("command")?),
        "suggest_command" => ClickEvent::SuggestCommand(field("command")?),
        "copy_to_clipboard" => ClickEvent::CopyToClipboard(field("value")?),
        "change_page" => {
            let page = required(fields, "page", int).into_result()?;
            if page <= 0 {
                return Err(format!("Value must be positive: {page}"));
            }
            ClickEvent::ChangePage(page)
        }
        "show_dialog" | "custom" => {
            let action = if action == "custom" { "custom" } else { "show_dialog" };
            ClickEvent::Other { action, fields: fields.iter().filter(|(k, _)| k != "action").cloned().collect() }
        }
        "open_file" => return Err("Click event type not allowed: OPEN_FILE".into()),
        other => return Err(format!("Unknown element name:{other}")),
    })
}

fn unknown_registry_key(registry: &str, raw: &str) -> String {
    match crate::resources::identifier::read(raw) {
        Ok(id) => format!("Unknown registry key in ResourceKey[minecraft:root / {registry}]: {id}"),
        Err(message) => message,
    }
}

fn nbt_uuid(tag: &Nbt) -> DataResult<u128> {
    match tag {
        Nbt::IntArray(ints) if ints.len() == 4 => DataResult::success(ints.iter().fold(0u128, |acc, &i| acc << 32 | i as u32 as u128)),
        Nbt::String(s) => match crate::util::java_uuid::from_string(s) {
            Ok(uuid) => DataResult::success(uuid),
            Err(message) => DataResult::error(format!("Invalid UUID {s}: {message}")),
        },
        other => DataResult::error(format!("Not an int array: {}", snbt(other))),
    }
}

fn json_uuid(tag: &Nbt) -> DataResult<u128> {
    let either = |first: String, second: String| DataResult::error(format!("Failed to parse either. First: {first}; Second: {second}"));
    match tag {
        Nbt::List(items) => {
            if items.len() > 4 && items.iter().all(|item| matches!(item, Nbt::Long(_) | Nbt::Double(_) | Nbt::LazilyParsedNumber(_))) {
                return DataResult::error("Input is not a list of 4 ints");
            }
            if !items.iter().all(|item| matches!(item, Nbt::Long(_) | Nbt::Double(_) | Nbt::LazilyParsedNumber(_))) {
                return either(format!("Some elements are not ints: {}", snbt(tag)), format!("Not a string: {}", snbt(tag)));
            }
            if items.len() != 4 {
                return either("Input is not a list of 4 ints".to_string(), format!("Not a string: {}", snbt(tag)));
            }
            DataResult::success(items.iter().fold(0u128, |acc, item| acc << 32 | int(item).value.unwrap_or(0) as u32 as u128))
        }
        Nbt::String(s) => match crate::util::java_uuid::from_string(s) {
            Ok(uuid) => DataResult::success(uuid),
            Err(message) => either(format!("Not a json array: {}", snbt(tag)), format!("Invalid UUID {s}: {message}")),
        },
        other => either(format!("Not a json array: {}", snbt(other)), format!("Not a string: {}", snbt(other))),
    }
}

fn hover_event(tag: &Nbt) -> DataResult<HoverEvent> {
    let Nbt::Compound(fields) = tag else { return DataResult::error(not_a_map(tag)) };
    let action = match required(fields, "action", string).into_result() {
        Ok(action) => action,
        Err(message) => return DataResult::error(message),
    };
    match action.as_str() {
        "show_text" => required(fields, "value", decode_component).map(|value| HoverEvent::ShowText(Box::new(value))),
        "show_item" => {
            let id = required(fields, "id", |raw| {
                string(raw).flat_map(|raw| match parse_identifier(&raw).and_then(|id| registry::ITEM.iter().find(|&&i| i == id)) {
                    Some(&"minecraft:air") => DataResult::error("Item must not be minecraft:air"),
                    Some(id) => DataResult::success(*id),
                    None => DataResult::error(unknown_registry_key("minecraft:item", &raw)),
                })
            });
            let count = optional(get(fields, "count"), |count| {
                int(count).flat_map(|count| {
                    if (1..=99).contains(&count) { DataResult::success(count) } else { DataResult::error(format!("Value must be within range [1;99]: {count}")) }
                })
            });
            let components = optional(get(fields, "components"), |components| {
                DataResult::from_result(DataComponentPatch::from_nbt(components).map(|patch| match patch.to_nbt() {
                    Nbt::Compound(fields) => fields,
                    _ => Vec::new(),
                }))
            });
            let error = record_error(&[&id.error, &count.error, &components.error]);
            let value = match (id.value, count.value, components.value) {
                (Some(id), Some(count), Some(components)) => Some(HoverEvent::ShowItem { id, count: count.unwrap_or(1), components: components.unwrap_or_default() }),
                _ => None,
            };
            DataResult { value, error }
        }
        "show_entity" => {
            let id = required(fields, "id", |raw| {
                string(raw).flat_map(|raw| match parse_identifier(&raw).and_then(|id| registry::ENTITY_TYPE.iter().find(|&&t| t == id)) {
                    Some(id) => DataResult::success(*id),
                    None => DataResult::error(unknown_registry_key("minecraft:entity_type", &raw)),
                })
            });
            let uuid = required(fields, "uuid", |uuid| if json() { json_uuid(uuid) } else { nbt_uuid(uuid) });
            let name = optional(get(fields, "name"), decode_component);
            let error = record_error(&[&id.error, &uuid.error, &name.error]);
            let value = match (id.value, uuid.value, name.value) {
                (Some(id), Some(uuid), Some(name)) => Some(HoverEvent::ShowEntity { id, uuid, name: name.map(Box::new) }),
                _ => None,
            };
            DataResult { value, error }
        }
        other => DataResult::error(format!("Unknown element name:{other}")),
    }
}
