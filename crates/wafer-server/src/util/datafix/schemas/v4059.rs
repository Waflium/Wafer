use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields, optional_fields_lazy, or};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register, remove};
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::v3818_3;

pub static V4059: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

pub fn components() -> TypeTemplates {
    let mut components = v3818_3::components();
    remove(&mut components, "minecraft:food");
    register(&mut components, "minecraft:use_remainder", |s| ITEM_STACK.in_schema(s));
    register(&mut components, "minecraft:equippable", |s| {
        optional_fields(vec![("allowed_entities", or(ENTITY_NAME.in_schema(s), list(ENTITY_NAME.in_schema(s))))])
    });
    register(&mut components, "minecraft:sulfur_cube_content", |s| ITEM_STACK.in_schema(s));
    components
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, DATA_COMPONENTS, |s| optional_fields_lazy(&components(), s));
}
