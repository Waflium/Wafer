use std::sync::Arc;

use crate::datafixers::dsl::TypeTemplates;
use crate::datafixers::schemas::schema::{Schema, SchemaClass, remove};

pub static V4885: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    remove(&mut map, "minecraft:bed");
    map
}
