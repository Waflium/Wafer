use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, hook, identity_hook, optional_fields};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass};
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::v99;

pub static V102: SchemaClass = SchemaClass {
    namespaced: false,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: None,
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(true, ITEM_STACK, |s| {
        hook(optional_fields(vec![("id", ITEM_NAME.in_schema(s)), ("tag", v99::item_stack_tag(s))]), v99::add_names_hook, identity_hook)
    });
}
