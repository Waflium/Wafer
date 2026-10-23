use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, hook, identity_hook, optional_fields};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;

pub static V3818_5: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

pub fn unpack_plain_id(value: Dynamic) -> Dynamic {
    if value.as_string().is_success() {
        let id = value.clone();
        let count = value.create_int(1);
        value.create_map([(value.create_string("id"), id), (value.create_string("count"), count)])
    } else {
        value
    }
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, ITEM_STACK, |s| {
        hook(optional_fields(vec![("id", ITEM_NAME.in_schema(s)), ("components", DATA_COMPONENTS.in_schema(s))]), unpack_plain_id, identity_hook)
    });
}
