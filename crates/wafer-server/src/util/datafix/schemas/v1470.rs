use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, optional_fields};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;

pub static V1470: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

pub fn register_mob(map: &mut TypeTemplates, name: &str) {
    register_simple(map, name);
}

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register_mob(&mut map, "minecraft:turtle");
    register_mob(&mut map, "minecraft:cod_mob");
    register_mob(&mut map, "minecraft:tropical_fish");
    register_mob(&mut map, "minecraft:salmon_mob");
    register_mob(&mut map, "minecraft:puffer_fish");
    register_mob(&mut map, "minecraft:phantom");
    register_mob(&mut map, "minecraft:dolphin");
    register_mob(&mut map, "minecraft:drowned");
    register(&mut map, "minecraft:trident", |s| {
        optional_fields(vec![("inBlockState", BLOCK_STATE.in_schema(s)), ("Trident", ITEM_STACK.in_schema(s))])
    });
    map
}
