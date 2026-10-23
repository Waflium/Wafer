use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields, optional_fields_with_rest};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V135: SchemaClass = SchemaClass {
    namespaced: false,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(false, PLAYER, |s| {
        optional_fields(vec![
            ("RootVehicle", optional_fields(vec![("Entity", ENTITY_TREE.in_schema(s))])),
            ("ender_pearls", list(ENTITY_TREE.in_schema(s))),
            ("Inventory", list(ITEM_STACK.in_schema(s))),
            ("EnderItems", list(ITEM_STACK.in_schema(s))),
        ])
    });
    registry.register_type(true, ENTITY_TREE, |s| {
        optional_fields_with_rest(vec![("Passengers", list(ENTITY_TREE.in_schema(s)))], ENTITY.in_schema(s))
    });
}
