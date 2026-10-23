use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V3325: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register(&mut map, "minecraft:item_display", |s| optional_fields(vec![("item", ITEM_STACK.in_schema(s))]));
    register(&mut map, "minecraft:block_display", |s| optional_fields(vec![("block_state", BLOCK_STATE.in_schema(s))]));
    register(&mut map, "minecraft:text_display", |s| optional_fields(vec![("text", TEXT_COMPONENT.in_schema(s))]));
    map
}
