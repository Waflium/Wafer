use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V808: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

pub fn register_inventory(map: &mut TypeTemplates, name: &str) {
    register(map, name, |s| optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s)))]));
}

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register_inventory(&mut map, "minecraft:shulker_box");
    map
}
