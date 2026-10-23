use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;

pub static V4302: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register_simple(&mut map, "minecraft:test_block");
    register(&mut map, "minecraft:test_instance_block", |s| {
        optional_fields(vec![
            ("data", optional_fields(vec![("error_message", TEXT_COMPONENT.in_schema(s))])),
            ("errors", list(optional_fields(vec![("text", TEXT_COMPONENT.in_schema(s))]))),
        ])
    });
    map
}
