use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, fields, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;

pub static V3689: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: Some(register_block_entities),
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register_simple(&mut map, "minecraft:breeze");
    register_simple(&mut map, "minecraft:wind_charge");
    register_simple(&mut map, "minecraft:breeze_wind_charge");
    map
}

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:trial_spawner", |s| {
        optional_fields(vec![
            ("spawn_potentials", list(fields(vec![("data", fields(vec![("entity", ENTITY_TREE.in_schema(s))]))]))),
            ("spawn_data", fields(vec![("entity", ENTITY_TREE.in_schema(s))])),
        ])
    });
    map
}
