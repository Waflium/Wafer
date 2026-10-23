use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, compound_list, fields, list, optional_fields, or, remainder};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V2842: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(false, CHUNK, |s| {
        optional_fields(vec![
            ("entities", list(ENTITY_TREE.in_schema(s))),
            ("block_entities", list(or(BLOCK_ENTITY.in_schema(s), remainder()))),
            ("block_ticks", list(fields(vec![("i", BLOCK_NAME.in_schema(s))]))),
            ("sections", list(
                optional_fields(vec![
                    ("biomes", optional_fields(vec![("palette", list(BIOME.in_schema(s)))])),
                    ("block_states", optional_fields(vec![("palette", list(BLOCK_STATE.in_schema(s)))])),
                ]),
            )),
            ("structures", optional_fields(vec![("starts", compound_list(STRUCTURE_FEATURE.in_schema(s)))])),
        ])
    });
}
