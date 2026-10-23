use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, remove};
use crate::util::datafix::fixes::references::*;

pub static V703: SchemaClass = SchemaClass {
    namespaced: false,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    remove(&mut map, "EntityHorse");
    register(&mut map, "Horse", |s| optional_fields(vec![("ArmorItem", ITEM_STACK.in_schema(s)), ("SaddleItem", ITEM_STACK.in_schema(s))]));
    register(&mut map, "Donkey", |s| optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s))), ("SaddleItem", ITEM_STACK.in_schema(s))]));
    register(&mut map, "Mule", |s| optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s))), ("SaddleItem", ITEM_STACK.in_schema(s))]));
    register(&mut map, "ZombieHorse", |s| optional_fields(vec![("SaddleItem", ITEM_STACK.in_schema(s))]));
    register(&mut map, "SkeletonHorse", |s| optional_fields(vec![("SaddleItem", ITEM_STACK.in_schema(s))]));
    map
}
