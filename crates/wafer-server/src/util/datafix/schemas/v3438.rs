use std::sync::Arc;

use crate::datafixers::dsl::TypeTemplates;
use crate::datafixers::schemas::schema::{Schema, SchemaClass, put, register_simple, remove};

pub static V3438: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    let removed = remove(&mut map, "minecraft:suspicious_sand").expect("minecraft:suspicious_sand");
    put(&mut map, "minecraft:brushable_block", removed);
    register_simple(&mut map, "minecraft:calibrated_sculk_sensor");
    map
}
