use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::named_entity_write_read_fix::NamedEntityWriteReadFix;
use crate::util::datafix::fixes::references::BLOCK_ENTITY;
use crate::util::datafix::legacy_component_data_fix_utils;

pub const FIELDS_TO_DROP: [&str; 10] = ["Text1", "Text2", "Text3", "Text4", "FilteredText1", "FilteredText2", "FilteredText3", "FilteredText4", "Color", "GlowingText"];
pub const FILTERED_CORRECT: &str = "_filtered_correct";

pub struct BlockEntitySignDoubleSidedEditableTextFix;

impl BlockEntitySignDoubleSidedEditableTextFix {
    pub fn new(output_schema: &Arc<Schema>, name: &str, entity_name: &str) -> NamedEntityWriteReadFix {
        NamedEntityWriteReadFix::new(output_schema, true, name, BLOCK_ENTITY, entity_name, fix)
    }
}

fn fix(mut input: Dynamic) -> Dynamic {
    let front_text = fix_front_text_tag(&input);
    let back_text = create_default_text(&input);
    let is_waxed = input.create_boolean(false);
    let filtered_correct = input.create_boolean(true);
    input = input.set("front_text", front_text).set("back_text", back_text).set("is_waxed", is_waxed).set(FILTERED_CORRECT, filtered_correct);
    for field in FIELDS_TO_DROP {
        input = input.remove(field);
    }
    input
}

fn fix_front_text_tag(tag: &Dynamic) -> Dynamic {
    let empty_line = legacy_component_data_fix_utils::create_empty_component();
    let lines: Vec<Dynamic> = get_lines(tag, "Text").into_iter().map(|line| line.unwrap_or_else(|| empty_line.clone())).collect();
    let color = tag.get("Color").result().unwrap_or_else(|| tag.create_string("black"));
    let has_glowing_text = tag.get("GlowingText").result().unwrap_or_else(|| tag.create_boolean(false));
    let mut text = tag.empty_map().set("messages", tag.create_list(lines.iter().cloned())).set("color", color).set("has_glowing_text", has_glowing_text);
    let filtered_lines = get_lines(tag, "FilteredText");
    if filtered_lines.iter().any(Option::is_some) {
        let filtered_messages = filtered_lines.into_iter().enumerate().map(|(index, line)| line.unwrap_or_else(|| lines[index].clone()));
        text = text.set("filtered_messages", tag.create_list(filtered_messages));
    }
    text
}

fn get_lines(tag: &Dynamic, line_prefix: &str) -> Vec<Option<Dynamic>> {
    vec![
        tag.get(&format!("{line_prefix}1")).result(),
        tag.get(&format!("{line_prefix}2")).result(),
        tag.get(&format!("{line_prefix}3")).result(),
        tag.get(&format!("{line_prefix}4")).result(),
    ]
}

fn create_default_text(tag: &Dynamic) -> Dynamic {
    let messages = create_empty_lines(tag);
    let color = tag.create_string("black");
    let has_glowing_text = tag.create_boolean(false);
    tag.empty_map().set("messages", messages).set("color", color).set("has_glowing_text", has_glowing_text)
}

fn create_empty_lines(tag: &Dynamic) -> Dynamic {
    let empty_component = legacy_component_data_fix_utils::create_empty_component();
    tag.create_list([empty_component.clone(), empty_component.clone(), empty_component.clone(), empty_component])
}
