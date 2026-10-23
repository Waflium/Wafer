use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::datafixers::typed::{find_field, type_finder};
use crate::util::datafix::fixes::named_entity_fix::NamedEntityFix;
use crate::util::datafix::fixes::references::{BLOCK_ENTITY, TEXT_COMPONENT};
use crate::util::datafix::legacy_component_data_fix_utils;

const LINE_FIELDS: [&str; 4] = ["Text1", "Text2", "Text3", "Text4"];

pub struct SignTextStrictJsonFix;

impl SignTextStrictJsonFix {
    pub fn new(output_schema: &Arc<Schema>) -> NamedEntityFix {
        let text_component_finder = type_finder(output_schema.get_type(TEXT_COMPONENT));
        NamedEntityFix::new(output_schema, false, "SignTextStrictJsonFix", BLOCK_ENTITY, "Sign", move |mut entity| {
            for line_field in LINE_FIELDS {
                let line_finder = find_field(entity.get_type(), line_field);
                entity = entity.update_typed(&line_finder, None, |line| {
                    line.update(&text_component_finder, |text_component| text_component.map_string(legacy_component_data_fix_utils::rewrite_from_lenient))
                });
            }
            entity
        })
    }
}
