use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, fields, list, optional_fields};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V106: SchemaClass = SchemaClass {
    namespaced: false,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, UNTAGGED_SPAWNER, |s| {
        optional_fields(vec![("SpawnPotentials", list(fields(vec![("Entity", ENTITY_TREE.in_schema(s))]))), ("SpawnData", ENTITY_TREE.in_schema(s))])
    });
}
