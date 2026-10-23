use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, optional_fields, optional_fields_lazy};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::v4059;

pub static V4996: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

pub fn components() -> TypeTemplates {
    let mut components = v4059::components();
    register(&mut components, "minecraft:pot_decorations", |s| {
        optional_fields(vec![
            ("back", ITEM_STACK.in_schema(s)),
            ("left", ITEM_STACK.in_schema(s)),
            ("right", ITEM_STACK.in_schema(s)),
            ("front", ITEM_STACK.in_schema(s)),
        ])
    });
    components
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, DATA_COMPONENTS, |s| optional_fields_lazy(&components(), s));
}
