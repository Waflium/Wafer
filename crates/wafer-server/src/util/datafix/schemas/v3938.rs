use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplate, TypeTemplates, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V3938: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

pub fn abstract_arrow(schema: &Schema) -> TypeTemplate {
    optional_fields(vec![
        ("inBlockState", BLOCK_STATE.in_schema(schema)),
        ("item", ITEM_STACK.in_schema(schema)),
        ("weapon", ITEM_STACK.in_schema(schema)),
    ])
}

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register(&mut map, "minecraft:spectral_arrow", |s| abstract_arrow(s));
    register(&mut map, "minecraft:arrow", |s| abstract_arrow(s));
    map
}
