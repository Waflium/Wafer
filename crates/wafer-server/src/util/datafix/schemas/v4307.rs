use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplate, TypeTemplates, list, optional_fields, optional_fields_lazy, or};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::v4059;

pub static V4307: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

pub fn components() -> TypeTemplates {
    let mut components = v4059::components();
    register(&mut components, "minecraft:can_place_on", |s| adventure_mode_predicate(s));
    register(&mut components, "minecraft:can_break", |s| adventure_mode_predicate(s));
    components
}

fn adventure_mode_predicate(schema: &Schema) -> TypeTemplate {
    let predicate = optional_fields(vec![("blocks", or(BLOCK_NAME.in_schema(schema), list(BLOCK_NAME.in_schema(schema))))]);
    or(predicate.clone(), list(predicate))
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, DATA_COMPONENTS, |s| optional_fields_lazy(&components(), s));
}
