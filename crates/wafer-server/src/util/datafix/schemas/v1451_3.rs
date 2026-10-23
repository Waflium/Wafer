use std::sync::Arc;

use crate::datafixers::dsl::{TypeTemplates, list, optional_fields, optional_fields_with_rest};
use crate::datafixers::schemas::schema::{Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;

pub static V1451_3: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: None,
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn register_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_entities();
    register_simple(&mut map, "minecraft:egg");
    register_simple(&mut map, "minecraft:ender_pearl");
    register_simple(&mut map, "minecraft:fireball");
    register(&mut map, "minecraft:potion", |s| optional_fields(vec![("Potion", ITEM_STACK.in_schema(s))]));
    register_simple(&mut map, "minecraft:small_fireball");
    register_simple(&mut map, "minecraft:snowball");
    register_simple(&mut map, "minecraft:wither_skull");
    register_simple(&mut map, "minecraft:xp_bottle");
    register(&mut map, "minecraft:arrow", |s| optional_fields(vec![("inBlockState", BLOCK_STATE.in_schema(s))]));
    register(&mut map, "minecraft:enderman", |s| optional_fields(vec![("carriedBlockState", BLOCK_STATE.in_schema(s))]));
    register(&mut map, "minecraft:falling_block", |s| {
        optional_fields(vec![("BlockState", BLOCK_STATE.in_schema(s)), ("TileEntityData", BLOCK_ENTITY.in_schema(s))])
    });
    register(&mut map, "minecraft:spectral_arrow", |s| optional_fields(vec![("inBlockState", BLOCK_STATE.in_schema(s))]));
    register(&mut map, "minecraft:chest_minecart", |s| {
        optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))])
    });
    register(&mut map, "minecraft:commandblock_minecart", |s| {
        optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s)), ("LastOutput", TEXT_COMPONENT.in_schema(s))])
    });
    register(&mut map, "minecraft:furnace_minecart", |s| optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s))]));
    register(&mut map, "minecraft:hopper_minecart", |s| {
        optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))])
    });
    register(&mut map, "minecraft:minecart", |s| optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s))]));
    register(&mut map, "minecraft:spawner_minecart", |s| {
        optional_fields_with_rest(vec![("DisplayState", BLOCK_STATE.in_schema(s))], UNTAGGED_SPAWNER.in_schema(s))
    });
    register(&mut map, "minecraft:tnt_minecart", |s| optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s))]));
    map
}
