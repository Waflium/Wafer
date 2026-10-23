use std::sync::Arc;

use crate::datafixers::dsl::TypeTemplates;
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register_simple, remove};

pub static V1928: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

pub fn register_mob(map: &mut TypeTemplates, name: &str) {
    register_simple(map, name);
}

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    remove(&mut map, "minecraft:illager_beast");
    register_mob(&mut map, "minecraft:ravager");
    map
}
