use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplate, TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;

pub static V4300: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register(&mut map, "minecraft:llama", |s| entity_with_inventory(s));
    register(&mut map, "minecraft:trader_llama", |s| entity_with_inventory(s));
    register(&mut map, "minecraft:donkey", |s| entity_with_inventory(s));
    register(&mut map, "minecraft:mule", |s| entity_with_inventory(s));
    register_simple(&mut map, "minecraft:horse");
    register_simple(&mut map, "minecraft:skeleton_horse");
    register_simple(&mut map, "minecraft:zombie_horse");
    map
}

fn entity_with_inventory(schema: &Schema) -> TypeTemplate {
    optional_fields(vec![("Items", list(ITEM_STACK.in_schema(schema)))])
}
