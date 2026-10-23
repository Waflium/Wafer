use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplates, hook, identity_hook, optional_fields, optional_fields_with_rest, tagged_choice_lazy};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, put, remove};
use crate::util::datafix::fixes::block_entity_id_fix::ID_MAP;
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::v99;

pub static V704: SchemaClass = SchemaClass {
    namespaced: false,
    namespaced_block_entity_choices: true,
    register_types: Some(register_types),
    register_entities: None,
    register_block_entities: Some(register_block_entities),
};

pub fn item_to_blockentity(item: &str) -> Option<&'static str> {
    Some(match item {
        "minecraft:furnace" | "minecraft:lit_furnace" => "minecraft:furnace",
        "minecraft:chest" | "minecraft:trapped_chest" => "minecraft:chest",
        "minecraft:ender_chest" => "minecraft:ender_chest",
        "minecraft:jukebox" => "minecraft:jukebox",
        "minecraft:dispenser" => "minecraft:dispenser",
        "minecraft:dropper" => "minecraft:dropper",
        "minecraft:sign" | "minecraft:standing_sign" | "minecraft:wall_sign" | "minecraft:oak_sign" | "minecraft:spruce_sign" | "minecraft:birch_sign" | "minecraft:jungle_sign" | "minecraft:acacia_sign" | "minecraft:dark_oak_sign" | "minecraft:crimson_sign" | "minecraft:warped_sign" => "minecraft:sign",
        "minecraft:mob_spawner" | "minecraft:spawner" => "minecraft:mob_spawner",
        "minecraft:noteblock" => "minecraft:noteblock",
        "minecraft:brewing_stand" => "minecraft:brewing_stand",
        "minecraft:enhanting_table" => "minecraft:enchanting_table",
        "minecraft:command_block" | "minecraft:repeating_command_block" | "minecraft:chain_command_block" => "minecraft:command_block",
        "minecraft:beacon" => "minecraft:beacon",
        "minecraft:skull" | "minecraft:skeleton_skull" | "minecraft:wither_skeleton_skull" | "minecraft:zombie_head" | "minecraft:player_head" | "minecraft:creeper_head" | "minecraft:dragon_head" => "minecraft:skull",
        "minecraft:daylight_detector" | "minecraft:daylight_detector_inverted" => "minecraft:daylight_detector",
        "minecraft:hopper" => "minecraft:hopper",
        "minecraft:banner" | "minecraft:white_banner" | "minecraft:orange_banner" | "minecraft:magenta_banner" | "minecraft:light_blue_banner" | "minecraft:yellow_banner" | "minecraft:lime_banner" | "minecraft:pink_banner" | "minecraft:gray_banner" | "minecraft:silver_banner" | "minecraft:light_gray_banner" | "minecraft:cyan_banner" | "minecraft:purple_banner" | "minecraft:blue_banner" | "minecraft:brown_banner" | "minecraft:green_banner" | "minecraft:red_banner" | "minecraft:black_banner" | "minecraft:wall_banner" | "minecraft:standing_banner" | "minecraft:shield" => "minecraft:banner",
        "minecraft:flower_pot" => "minecraft:flower_pot",
        "minecraft:shulker_box" | "minecraft:white_shulker_box" | "minecraft:orange_shulker_box" | "minecraft:magenta_shulker_box" | "minecraft:light_blue_shulker_box" | "minecraft:yellow_shulker_box" | "minecraft:lime_shulker_box" | "minecraft:pink_shulker_box" | "minecraft:gray_shulker_box" | "minecraft:silver_shulker_box" | "minecraft:cyan_shulker_box" | "minecraft:purple_shulker_box" | "minecraft:blue_shulker_box" | "minecraft:brown_shulker_box" | "minecraft:green_shulker_box" | "minecraft:red_shulker_box" | "minecraft:black_shulker_box" | "minecraft:light_gray_shulker_box" => "minecraft:shulker_box",
        "minecraft:bed" | "minecraft:white_bed" | "minecraft:orange_bed" | "minecraft:magenta_bed" | "minecraft:light_blue_bed" | "minecraft:yellow_bed" | "minecraft:lime_bed" | "minecraft:pink_bed" | "minecraft:gray_bed" | "minecraft:silver_bed" | "minecraft:light_gray_bed" | "minecraft:cyan_bed" | "minecraft:purple_bed" | "minecraft:blue_bed" | "minecraft:brown_bed" | "minecraft:green_bed" | "minecraft:red_bed" | "minecraft:black_bed" => "minecraft:bed",
        "minecraft:piston_head" => "minecraft:piston",
        "minecraft:unpowered_comparator" | "minecraft:powered_comparator" => "minecraft:comparator",
        "minecraft:structure_block" => "minecraft:structure_block",
        "minecraft:end_portal" => "minecraft:end_portal",
        "minecraft:end_gateway" => "minecraft:end_gateway",
        "minecraft:barrel" => "minecraft:barrel",
        "minecraft:conduit" => "minecraft:conduit",
        "minecraft:smoker" => "minecraft:smoker",
        "minecraft:blast_furnace" => "minecraft:blast_furnace",
        "minecraft:lectern" => "minecraft:lectern",
        "minecraft:bell" => "minecraft:bell",
        "minecraft:jigsaw" => "minecraft:jigsaw",
        "minecraft:campfire" => "minecraft:campfire",
        "minecraft:bee_nest" | "minecraft:beehive" => "minecraft:beehive",
        "minecraft:sculk_sensor" => "minecraft:sculk_sensor",
        "minecraft:decorated_pot" => "minecraft:decorated_pot",
        "minecraft:crafter" => "minecraft:crafter",
        _ => return None,
    })
}

pub fn add_names_hook(value: Dynamic) -> Dynamic {
    v99::add_names(value, item_to_blockentity, v99::item_to_entity)
}

fn register_block_entities(parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = parent.expect("a parent schema").register_block_entities();
    for (old_id, new_id) in ID_MAP {
        let removed = remove(&mut map, old_id).unwrap_or_else(|| panic!("Didn't find {old_id} in schema"));
        put(&mut map, new_id, removed);
    }
    map
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    let block_entities = block_entity_types.clone();
    registry.register_type(true, BLOCK_ENTITY, move |s| {
        optional_fields_with_rest(
            vec![("components", DATA_COMPONENTS.in_schema(s))],
            tagged_choice_lazy("id", ConstType::NamespacedString, block_entities.clone()),
        )
    });
    registry.register_type(true, ITEM_STACK, |s| {
        hook(optional_fields(vec![("id", ITEM_NAME.in_schema(s)), ("tag", v99::item_stack_tag(s))]), add_names_hook, identity_hook)
    });
}
