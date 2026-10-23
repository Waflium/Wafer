use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V3327: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:decorated_pot", |s| {
        optional_fields(vec![("shards", list(ITEM_NAME.in_schema(s))), ("item", ITEM_STACK.in_schema(s))])
    });
    register(&mut map, "minecraft:suspicious_sand", |s| optional_fields(vec![("item", ITEM_STACK.in_schema(s))]));
    map
}
