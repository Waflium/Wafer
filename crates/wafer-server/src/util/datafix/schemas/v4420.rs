use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V4420: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register(&mut map, "minecraft:area_effect_cloud", |s| optional_fields(vec![("custom_particle", PARTICLE.in_schema(s))]));
    map
}
