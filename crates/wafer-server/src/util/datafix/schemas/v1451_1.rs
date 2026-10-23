use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, fields, list, optional_fields, or, remainder};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V1451_1: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(false, CHUNK, |s| {
        fields(vec![
            ("Level", optional_fields(vec![
                ("Entities", list(ENTITY_TREE.in_schema(s))),
                ("TileEntities", list(or(BLOCK_ENTITY.in_schema(s), remainder()))),
                ("TileTicks", list(fields(vec![("i", BLOCK_NAME.in_schema(s))]))),
                ("Sections", list(optional_fields(vec![("Palette", list(BLOCK_STATE.in_schema(s)))]))),
            ])),
        ])
    });
}
