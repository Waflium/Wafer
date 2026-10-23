use std::sync::Arc;

use crate::datafixers::dsl::TypeTemplates;
use crate::datafixers::schemas::schema::{Schema, SchemaClass, put, remove};

pub static V1483: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    let removed = remove(&mut map, "minecraft:puffer_fish").expect("minecraft:puffer_fish");
    put(&mut map, "minecraft:pufferfish", removed);
    map
}
