use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, remove};
use crate::util::datafix::fixes::references::*;

pub static V4306: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    remove(&mut map, "minecraft:potion");
    register(&mut map, "minecraft:splash_potion", |s| optional_fields(vec![("Item", ITEM_STACK.in_schema(s))]));
    register(&mut map, "minecraft:lingering_potion", |s| optional_fields(vec![("Item", ITEM_STACK.in_schema(s))]));
    map
}
