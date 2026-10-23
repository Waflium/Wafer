use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V3083: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register(&mut map, "minecraft:allay", |s| {
        optional_fields(vec![
            ("Inventory", list(ITEM_STACK.in_schema(s))),
            ("listener", optional_fields(vec![("event", optional_fields(vec![("game_event", GAME_EVENT_NAME.in_schema(s))]))])),
        ])
    });
    map
}
