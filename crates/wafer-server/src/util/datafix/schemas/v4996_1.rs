use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V4996_1: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:decorated_pot", |s| {
        optional_fields(vec![
            ("sherds", optional_fields(vec![
                ("back", ITEM_STACK.in_schema(s)),
                ("left", ITEM_STACK.in_schema(s)),
                ("right", ITEM_STACK.in_schema(s)),
                ("front", ITEM_STACK.in_schema(s)),
            ])),
            ("item", ITEM_STACK.in_schema(s)),
        ])
    });
    map
}
