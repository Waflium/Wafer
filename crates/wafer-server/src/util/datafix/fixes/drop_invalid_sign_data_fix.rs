use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::named_choice;
use crate::util::datafix::fixes::block_entity_sign_double_sided_editable_text_fix::FIELDS_TO_DROP;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;
use crate::util::datafix::legacy_component_data_fix_utils;

pub struct DropInvalidSignDataFix {
    base: DataFixBase,
    entity_name: String,
}

impl DropInvalidSignDataFix {
    pub fn new(output_schema: &Arc<Schema>, entity_name: &str) -> DropInvalidSignDataFix {
        DropInvalidSignDataFix { base: DataFixBase::new(output_schema, false), entity_name: entity_name.to_string() }
    }
}

fn fix(mut tag: Dynamic) -> Dynamic {
    tag = tag.update("front_text", fix_text);
    tag = tag.update("back_text", fix_text);
    for field in FIELDS_TO_DROP {
        tag = tag.remove(field);
    }
    tag
}

fn fix_text(tag: Dynamic) -> Dynamic {
    let Some(filtered_lines) = tag.get("filtered_messages").as_stream_opt().into_result().ok() else { return tag };
    let empty_component = legacy_component_data_fix_utils::create_empty_component();
    let lines = tag.get("messages").as_stream_opt().into_result().ok().unwrap_or_default();
    let new_filtered_lines: Vec<Dynamic> = filtered_lines
        .into_iter()
        .enumerate()
        .map(|(index, line)| {
            let fallback_line = if index < lines.len() { lines[index].clone() } else { empty_component.clone() };
            if line == empty_component { fallback_line } else { line }
        })
        .collect();
    if new_filtered_lines == lines {
        tag.remove("filtered_messages")
    } else {
        let filtered_messages = tag.create_list(new_filtered_lines);
        tag.set("filtered_messages", filtered_messages)
    }
}

impl DataFix for DropInvalidSignDataFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let entity_type = self.get_input_schema().get_type(BLOCK_ENTITY);
        let entity_choice_type = self.get_input_schema().get_choice_type(BLOCK_ENTITY, &self.entity_name);
        let entity_f = named_choice(&self.entity_name, entity_choice_type.clone());
        self.fix_type_everywhere_typed(&format!("DropInvalidSignDataFix for {}", self.entity_name), entity_type, move |input| {
            input.update_typed(&entity_f, Some(&entity_choice_type), |entity| {
                let filtered_correct = entity.get_remainder().get("_filtered_correct").as_boolean_or(false);
                if filtered_correct {
                    entity.update_remainder(|remainder| remainder.remove("_filtered_correct"))
                } else {
                    crate::util::write_and_read_typed_or_throw(&entity, &entity_choice_type, fix)
                }
            })
        })
    }
}
