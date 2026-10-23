use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplates, list, optional_fields, or, string, tagged_choice};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V4290: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, TEXT_COMPONENT, |s| {
        or(
            or(string(), list(TEXT_COMPONENT.in_schema(s))),
            optional_fields(vec![
                ("extra", list(TEXT_COMPONENT.in_schema(s))),
                ("separator", TEXT_COMPONENT.in_schema(s)),
                ("hoverEvent", tagged_choice(
                    "action",
                    ConstType::String,
                    vec![
                        ("show_text", optional_fields(vec![("contents", TEXT_COMPONENT.in_schema(s))])),
                        ("show_item", optional_fields(vec![("contents", or(ITEM_STACK.in_schema(s), ITEM_NAME.in_schema(s)))])),
                        ("show_entity", optional_fields(vec![("type", ENTITY_NAME.in_schema(s)), ("name", TEXT_COMPONENT.in_schema(s))])),
                    ],
                )),
            ]),
        )
    });
}
