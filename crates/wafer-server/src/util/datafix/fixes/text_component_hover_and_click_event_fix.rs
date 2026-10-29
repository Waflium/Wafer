use wafer_core::java_string::JavaStr;
use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::Typed;
use crate::datafixers::types::{Type, and_all, const_type, equals, field, list, named, optional, or, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::extra_data_fix_utils;
use crate::util::datafix::fixes::references::TEXT_COMPONENT;
use crate::util::java_integer::parse_int;
use crate::util::uri::URI;

pub struct TextComponentHoverAndClickEventFix {
    base: DataFixBase,
}

impl TextComponentHoverAndClickEventFix {
    pub fn new(output_schema: &Arc<Schema>) -> TextComponentHoverAndClickEventFix {
        TextComponentHoverAndClickEventFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for TextComponentHoverAndClickEventFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let hover_event_type = self.get_input_schema().get_type(TEXT_COMPONENT).find_field_type("hoverEvent").expect("a hoverEvent field");
        self.create_fixer(self.get_input_schema().get_type_raw(TEXT_COMPONENT), self.get_output_schema().get_type(TEXT_COMPONENT), hover_event_type)
    }
}

impl TextComponentHoverAndClickEventFix {
    fn create_fixer(&self, old_raw_text_component_type: Arc<Type>, new_text_component_type: Arc<Type>, hover_event_type: Arc<Type>) -> TypeRewriteRule {
        let old_text_component_type = named(
            TEXT_COMPONENT.type_name(),
            or(
                or(const_type(ConstType::String), list(old_raw_text_component_type.clone())),
                and_all(vec![
                    optional(field("extra", list(old_raw_text_component_type.clone()))),
                    optional(field("separator", old_raw_text_component_type)),
                    optional(field("hoverEvent", hover_event_type)),
                    remainder_type(),
                ]),
            ),
        );
        let input_type = self.get_input_schema().get_type(TEXT_COMPONENT);
        if !equals(&old_text_component_type, &input_type, false) {
            panic!("Text component type did not match, expected {old_text_component_type:?} but got {input_type:?}");
        }
        let patched_input_type = extra_data_fix_utils::patch_sub_type(&old_text_component_type, &old_text_component_type, &new_text_component_type);
        let output_type = new_text_component_type.clone();
        self.fix_type_everywhere_to("TextComponentHoverAndClickEventFix", old_text_component_type, new_text_component_type, move |text_component| {
            let has_hover_or_click = match &text_component {
                Value::Left(_) => false,
                Value::Right(full) => {
                    let Value::Pair(_, separator_hover_remainder) = &**full else { panic!("Unexpected text component value") };
                    let Value::Pair(_, hover_and_remainder) = &**separator_hover_remainder else { panic!("Unexpected text component value") };
                    let Value::Pair(hover, remainder) = &**hover_and_remainder else { panic!("Unexpected text component value") };
                    let has_hover = matches!(&**hover, Value::Left(_));
                    let has_click = match &**remainder {
                        Value::Dynamic(remainder) => remainder.get("clickEvent").result().is_some(),
                        _ => false,
                    };
                    has_hover || has_click
                }
                other => panic!("Unexpected text component value {other:?}"),
            };
            if !has_hover_or_click {
                return text_component;
            }
            crate::util::write_and_read_typed_or_throw(&Typed::new(patched_input_type.clone(), text_component), &output_type, fix_text_component).value
        })
    }
}

fn fix_text_component(dynamic: Dynamic) -> Dynamic {
    let hover_event = dynamic.get("hoverEvent").result().map(fix_hover_event);
    let dynamic = dynamic.clone().remove("hoverEvent").set_field_if_present("hover_event", hover_event);
    let click_event = dynamic.get("clickEvent").result().and_then(fix_click_event);
    dynamic.remove("clickEvent").set_field_if_present("click_event", click_event)
}

fn copy_fields(mut target: Dynamic, source: &Dynamic, fields: &[&str]) -> Dynamic {
    for field in fields {
        target = Dynamic::copy_field(source, field, target, field);
    }
    target
}

fn fix_hover_event(dynamic: Dynamic) -> Dynamic {
    let action = dynamic.get("action").as_string_or("");
    match action.as_str() {
        "show_text" => dynamic.rename_field("contents", "value"),
        "show_item" => {
            let contents = dynamic.get("contents").or_else_empty_map();
            copy_fields(dynamic.remove("contents"), &contents, &["id", "count", "components"])
        }
        "show_entity" => {
            let contents = dynamic.get("contents").or_else_empty_map();
            copy_fields(dynamic.remove("contents"), &contents, &["id", "type", "name"]).rename_field("id", "uuid").rename_field("type", "id")
        }
        _ => dynamic,
    }
}

fn fix_click_event(dynamic: Dynamic) -> Option<Dynamic> {
    let action = dynamic.get("action").as_string_or("");
    let value = dynamic.get("value").as_string_or("");
    match action.as_str() {
        "open_url" => if !validate_uri(&value) { None } else { Some(dynamic.rename_field("value", "url")) },
        "open_file" => Some(dynamic.rename_field("value", "path")),
        "run_command" | "suggest_command" => if !validate_chat(&value) { None } else { Some(dynamic.rename_field("value", "command")) },
        "change_page" => {
            let old_page = dynamic.get("value").result().and_then(|value| parse_old_page(&value))?;
            let page = old_page.max(1);
            Some(dynamic.remove("value").set("page", Dynamic::new(wafer_protocol::nbt::Nbt::Int(page))))
        }
        _ => Some(dynamic),
    }
}

fn parse_old_page(value: &Dynamic) -> Option<i32> {
    if let Ok(number_value) = value.as_number().into_result() {
        return Some(number_value.int_value());
    }
    parse_int(&value.as_string_or(""))
}

fn validate_uri(uri: &str) -> bool {
    match URI::new(uri) {
        Ok(parsed_uri) => match parsed_uri.scheme() {
            Some(scheme) => {
                let protocol = scheme.to_lowercase();
                protocol == "http" || protocol == "https"
            }
            None => false,
        },
        Err(_) => false,
    }
}

fn validate_chat(string: &str) -> bool {
    for c in string.java_utf16() {
        if c == 167 || c < ' ' as u16 || c == 127 {
            return false;
        }
    }
    true
}
