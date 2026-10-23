use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V1451_2: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:piston", |s| optional_fields(vec![("blockState", BLOCK_STATE.in_schema(s))]));
    map
}
