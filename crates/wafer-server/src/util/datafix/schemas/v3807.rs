use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V3807: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register(&mut map, "minecraft:vault", |s| {
        optional_fields(vec![
            ("config", optional_fields(vec![("key_item", ITEM_STACK.in_schema(s))])),
            ("server_data", optional_fields(vec![("items_to_eject", list(ITEM_STACK.in_schema(s)))])),
            ("shared_data", optional_fields(vec![("display_item", ITEM_STACK.in_schema(s))])),
        ])
    });
    map
}
