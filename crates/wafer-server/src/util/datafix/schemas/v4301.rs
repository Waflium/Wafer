use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, field, optional, optional_fields};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V4301: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, ENTITY_EQUIPMENT, |s| {
        optional(
            field(
                "equipment",
                optional_fields(vec![
                    ("mainhand", ITEM_STACK.in_schema(s)),
                    ("offhand", ITEM_STACK.in_schema(s)),
                    ("feet", ITEM_STACK.in_schema(s)),
                    ("legs", ITEM_STACK.in_schema(s)),
                    ("chest", ITEM_STACK.in_schema(s)),
                    ("head", ITEM_STACK.in_schema(s)),
                    ("body", ITEM_STACK.in_schema(s)),
                    ("saddle", ITEM_STACK.in_schema(s)),
                ]),
            ),
        )
    });
}
