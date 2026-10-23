use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register};
use crate::util::datafix::fixes::references::*;

pub static V1929: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register(&mut map, "minecraft:wandering_trader", |s| {
        optional_fields(vec![
            ("Inventory", list(ITEM_STACK.in_schema(s))),
            ("Offers", optional_fields(vec![("Recipes", list(VILLAGER_TRADE.in_schema(s)))])),
        ])
    });
    register(&mut map, "minecraft:trader_llama", |s| {
        optional_fields(vec![
            ("Items", list(ITEM_STACK.in_schema(s))),
            ("SaddleItem", ITEM_STACK.in_schema(s)),
            ("DecorItem", ITEM_STACK.in_schema(s)),
        ])
    });
    map
}
