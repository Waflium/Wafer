use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplates, const_type, optional_fields};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V2684: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    registry.register_type(false, GAME_EVENT_NAME, |_| const_type(ConstType::NamespacedString));
}

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:sculk_sensor", |s| {
        optional_fields(vec![("listener", optional_fields(vec![("event", optional_fields(vec![("game_event", GAME_EVENT_NAME.in_schema(s))]))]))])
    });
    map
}
