use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplate, TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V3439: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:sign", |s| sign(s));
    map
}

pub fn sign(schema: &Schema) -> TypeTemplate {
    optional_fields(vec![
        ("front_text", optional_fields(vec![
            ("messages", list(TEXT_COMPONENT.in_schema(schema))),
            ("filtered_messages", list(TEXT_COMPONENT.in_schema(schema))),
        ])),
        ("back_text", optional_fields(vec![
            ("messages", list(TEXT_COMPONENT.in_schema(schema))),
            ("filtered_messages", list(TEXT_COMPONENT.in_schema(schema))),
        ])),
    ])
}
