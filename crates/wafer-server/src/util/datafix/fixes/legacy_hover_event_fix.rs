use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dsl::ConstType;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::json_ops;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::types::{Type, and_all, const_type, equals, field, list, named, optional, or, remainder_type};
use crate::datafixers::value::Value;
use crate::util::datafix::fixes::references::TEXT_COMPONENT;
use crate::util::gson;

pub struct LegacyHoverEventFix {
    base: DataFixBase,
}

impl LegacyHoverEventFix {
    pub fn new(output_schema: &Arc<Schema>) -> LegacyHoverEventFix {
        LegacyHoverEventFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for LegacyHoverEventFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let hover_event_type = self.get_input_schema().get_type(TEXT_COMPONENT).find_field_type("hoverEvent").expect("a hoverEvent field");
        self.create_fixer(self.get_input_schema().get_type_raw(TEXT_COMPONENT), hover_event_type)
    }
}

impl LegacyHoverEventFix {
    fn create_fixer(&self, raw_text_component_type: Arc<Type>, hover_event_type: Arc<Type>) -> TypeRewriteRule {
        let text_component_type = named(
            TEXT_COMPONENT.type_name(),
            or(
                or(const_type(ConstType::String), list(raw_text_component_type.clone())),
                and_all(vec![
                    optional(field("extra", list(raw_text_component_type.clone()))),
                    optional(field("separator", raw_text_component_type)),
                    optional(field("hoverEvent", hover_event_type.clone())),
                    remainder_type(),
                ]),
            ),
        );
        let input_type = self.get_input_schema().get_type(TEXT_COMPONENT);
        if !equals(&text_component_type, &input_type, false) {
            panic!("Text component type did not match, expected {text_component_type:?} but got {input_type:?}");
        }
        self.fix_type_everywhere("LegacyHoverEventFix", text_component_type, move |named| match named {
            Value::Right(full) => {
                let Value::Pair(extra, separator_hover_remainder) = *full else { panic!("Unexpected text component value") };
                let Value::Pair(separator, hover_and_remainder) = *separator_hover_remainder else { panic!("Unexpected text component value") };
                Value::Right(Box::new(Value::Pair(extra, Box::new(Value::Pair(separator, Box::new(fix_hover_and_remainder(&hover_event_type, *hover_and_remainder)))))))
            }
            other => other,
        })
    }
}

fn fix_hover_and_remainder(hover_event_type: &Arc<Type>, hover_and_remainder: Value) -> Value {
    let Value::Pair(hover, remainder) = hover_and_remainder else { panic!("Unexpected text component value") };
    let Value::Dynamic(remainder) = *remainder else { panic!("Unexpected text component value") };
    let Some(hover_event) = remainder.get("hoverEvent").result() else { return Value::Pair(hover, Box::new(Value::Dynamic(remainder))) };
    if hover_event.get("value").result().is_none() {
        return Value::Pair(hover, Box::new(Value::Dynamic(remainder)));
    }
    let hover_action = match &*hover {
        Value::Left(hover_event) => match &**hover_event {
            Value::Choice(action, _) => action.as_str(),
            _ => "",
        },
        _ => "",
    };
    let new_hover_event = fix_hover_event(hover_event_type, hover_action, hover_event);
    Value::Pair(Box::new(Value::Left(Box::new(new_hover_event))), Box::new(Value::Dynamic(remainder)))
}

fn fix_hover_event(hover_event_type: &Arc<Type>, action: &str, old_hover_event: Dynamic) -> Value {
    if action == "show_text" { fix_show_text_hover(hover_event_type, old_hover_event) } else { create_placeholder_hover(hover_event_type, old_hover_event) }
}

fn fix_show_text_hover(hover_event_type: &Arc<Type>, old_hover_event: Dynamic) -> Value {
    let new_hover_event = old_hover_event.rename_field("value", "contents");
    crate::util::read_typed_or_throw(hover_event_type, new_hover_event, false).value
}

fn create_placeholder_hover(hover_event_type: &Arc<Type>, old_hover_event: Dynamic) -> Value {
    let old_json = json_ops::convert_nbt_to_element(old_hover_event.get_value().expect("a hover event"));
    let text = format!("Legacy hoverEvent: {}", gson::to_stable_string(&old_json));
    let placeholder_hover_event = Dynamic::new(Nbt::Compound(vec![
        ("action".to_string(), Nbt::String("show_text".to_string())),
        ("contents".to_string(), Nbt::Compound(vec![("text".to_string(), Nbt::String(text))])),
    ]));
    crate::util::read_typed_or_throw(hover_event_type, placeholder_hover_event, false).value
}
