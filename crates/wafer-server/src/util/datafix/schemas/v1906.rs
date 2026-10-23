use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::v1458;

pub static V1906: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    register_inventory(&mut map, "minecraft:barrel");
    register_inventory(&mut map, "minecraft:smoker");
    register_inventory(&mut map, "minecraft:blast_furnace");
    register(&mut map, "minecraft:lectern", |s| optional_fields(vec![("Book", ITEM_STACK.in_schema(s))]));
    register_simple(&mut map, "minecraft:bell");
    map
}

pub fn register_inventory(map: &mut TypeTemplates, name: &str) {
    register(map, name, |s| v1458::nameable_inventory(s));
}
