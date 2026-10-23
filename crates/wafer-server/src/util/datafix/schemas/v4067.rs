use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple, remove};
use crate::util::datafix::fixes::references::*;

pub static V4067: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    remove(&mut map, "minecraft:boat");
    remove(&mut map, "minecraft:chest_boat");
    register_simple(&mut map, "minecraft:oak_boat");
    register_simple(&mut map, "minecraft:spruce_boat");
    register_simple(&mut map, "minecraft:birch_boat");
    register_simple(&mut map, "minecraft:jungle_boat");
    register_simple(&mut map, "minecraft:acacia_boat");
    register_simple(&mut map, "minecraft:cherry_boat");
    register_simple(&mut map, "minecraft:dark_oak_boat");
    register_simple(&mut map, "minecraft:mangrove_boat");
    register_simple(&mut map, "minecraft:bamboo_raft");
    register_chest_boat(&mut map, "minecraft:oak_chest_boat");
    register_chest_boat(&mut map, "minecraft:spruce_chest_boat");
    register_chest_boat(&mut map, "minecraft:birch_chest_boat");
    register_chest_boat(&mut map, "minecraft:jungle_chest_boat");
    register_chest_boat(&mut map, "minecraft:acacia_chest_boat");
    register_chest_boat(&mut map, "minecraft:cherry_chest_boat");
    register_chest_boat(&mut map, "minecraft:dark_oak_chest_boat");
    register_chest_boat(&mut map, "minecraft:mangrove_chest_boat");
    register_chest_boat(&mut map, "minecraft:bamboo_chest_raft");
    map
}

fn register_chest_boat(map: &mut TypeTemplates, id: &str) {
    register(map, id, |s| optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s)))]));
}
