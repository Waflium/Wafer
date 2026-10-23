use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;

pub static V2100: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: Some(register_block_entities),
};

pub fn register_mob(map: &mut TypeTemplates, name: &str) {
    register_simple(map, name);
}

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register_mob(&mut map, "minecraft:bee");
    register_mob(&mut map, "minecraft:bee_stinger");
    map
}

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:beehive", |s| {
        optional_fields(vec![("Bees", list(optional_fields(vec![("EntityData", ENTITY_TREE.in_schema(s))])))])
    });
    map
}
