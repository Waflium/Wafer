use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, and_all, field, list, optional};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V100: SchemaClass = SchemaClass {
    namespaced: false,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, ENTITY_EQUIPMENT, |s| {
        and_all(vec![
            optional(field("ArmorItems", list(ITEM_STACK.in_schema(s)))),
            optional(field("HandItems", list(ITEM_STACK.in_schema(s)))),
            optional(field("body_armor_item", ITEM_STACK.in_schema(s))),
            optional(field("saddle", ITEM_STACK.in_schema(s))),
        ])
    });
}
