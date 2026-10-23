use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;

pub static V4997: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register_simple(&mut map, "minecraft:poplar_boat");
    register(&mut map, "minecraft:poplar_chest_boat", |s| optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s)))]));
    map
}
