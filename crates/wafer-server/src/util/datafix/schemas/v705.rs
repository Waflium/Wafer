use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplates, and, hook, identity_hook, list, optional_fields, optional_fields_with_rest, string, tagged_choice_lazy};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::{v704, v99};

pub static V705: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: Some(register_entities),
    register_block_entities: None,
};

fn item_to_entity(item: &str) -> Option<&'static str> {
    Some(match item {
        "minecraft:armor_stand" => "minecraft:armor_stand",
        "minecraft:painting" => "minecraft:painting",
        "minecraft:armadillo_spawn_egg" => "minecraft:armadillo",
        "minecraft:allay_spawn_egg" => "minecraft:allay",
        "minecraft:axolotl_spawn_egg" => "minecraft:axolotl",
        "minecraft:bat_spawn_egg" => "minecraft:bat",
        "minecraft:bee_spawn_egg" => "minecraft:bee",
        "minecraft:blaze_spawn_egg" => "minecraft:blaze",
        "minecraft:bogged_spawn_egg" => "minecraft:bogged",
        "minecraft:breeze_spawn_egg" => "minecraft:breeze",
        "minecraft:cat_spawn_egg" => "minecraft:cat",
        "minecraft:camel_spawn_egg" => "minecraft:camel",
        "minecraft:cave_spider_spawn_egg" => "minecraft:cave_spider",
        "minecraft:chicken_spawn_egg" => "minecraft:chicken",
        "minecraft:cod_spawn_egg" => "minecraft:cod",
        "minecraft:cow_spawn_egg" => "minecraft:cow",
        "minecraft:creeper_spawn_egg" => "minecraft:creeper",
        "minecraft:dolphin_spawn_egg" => "minecraft:dolphin",
        "minecraft:donkey_spawn_egg" => "minecraft:donkey",
        "minecraft:drowned_spawn_egg" => "minecraft:drowned",
        "minecraft:elder_guardian_spawn_egg" => "minecraft:elder_guardian",
        "minecraft:ender_dragon_spawn_egg" => "minecraft:ender_dragon",
        "minecraft:enderman_spawn_egg" => "minecraft:enderman",
        "minecraft:endermite_spawn_egg" => "minecraft:endermite",
        "minecraft:evoker_spawn_egg" => "minecraft:evoker",
        "minecraft:fox_spawn_egg" => "minecraft:fox",
        "minecraft:frog_spawn_egg" => "minecraft:frog",
        "minecraft:ghast_spawn_egg" => "minecraft:ghast",
        "minecraft:glow_squid_spawn_egg" => "minecraft:glow_squid",
        "minecraft:goat_spawn_egg" => "minecraft:goat",
        "minecraft:guardian_spawn_egg" => "minecraft:guardian",
        "minecraft:hoglin_spawn_egg" => "minecraft:hoglin",
        "minecraft:horse_spawn_egg" => "minecraft:horse",
        "minecraft:husk_spawn_egg" => "minecraft:husk",
        "minecraft:iron_golem_spawn_egg" => "minecraft:iron_golem",
        "minecraft:llama_spawn_egg" => "minecraft:llama",
        "minecraft:magma_cube_spawn_egg" => "minecraft:magma_cube",
        "minecraft:mooshroom_spawn_egg" => "minecraft:mooshroom",
        "minecraft:mule_spawn_egg" => "minecraft:mule",
        "minecraft:ocelot_spawn_egg" => "minecraft:ocelot",
        "minecraft:panda_spawn_egg" => "minecraft:panda",
        "minecraft:parrot_spawn_egg" => "minecraft:parrot",
        "minecraft:phantom_spawn_egg" => "minecraft:phantom",
        "minecraft:pig_spawn_egg" => "minecraft:pig",
        "minecraft:piglin_spawn_egg" => "minecraft:piglin",
        "minecraft:piglin_brute_spawn_egg" => "minecraft:piglin_brute",
        "minecraft:pillager_spawn_egg" => "minecraft:pillager",
        "minecraft:polar_bear_spawn_egg" => "minecraft:polar_bear",
        "minecraft:pufferfish_spawn_egg" => "minecraft:pufferfish",
        "minecraft:rabbit_spawn_egg" => "minecraft:rabbit",
        "minecraft:ravager_spawn_egg" => "minecraft:ravager",
        "minecraft:salmon_spawn_egg" => "minecraft:salmon",
        "minecraft:sheep_spawn_egg" => "minecraft:sheep",
        "minecraft:shulker_spawn_egg" => "minecraft:shulker",
        "minecraft:silverfish_spawn_egg" => "minecraft:silverfish",
        "minecraft:skeleton_spawn_egg" => "minecraft:skeleton",
        "minecraft:skeleton_horse_spawn_egg" => "minecraft:skeleton_horse",
        "minecraft:slime_spawn_egg" => "minecraft:slime",
        "minecraft:sniffer_spawn_egg" => "minecraft:sniffer",
        "minecraft:snow_golem_spawn_egg" => "minecraft:snow_golem",
        "minecraft:spider_spawn_egg" => "minecraft:spider",
        "minecraft:squid_spawn_egg" => "minecraft:squid",
        "minecraft:stray_spawn_egg" => "minecraft:stray",
        "minecraft:strider_spawn_egg" => "minecraft:strider",
        "minecraft:tadpole_spawn_egg" => "minecraft:tadpole",
        "minecraft:trader_llama_spawn_egg" => "minecraft:trader_llama",
        "minecraft:tropical_fish_spawn_egg" => "minecraft:tropical_fish",
        "minecraft:turtle_spawn_egg" => "minecraft:turtle",
        "minecraft:vex_spawn_egg" => "minecraft:vex",
        "minecraft:villager_spawn_egg" => "minecraft:villager",
        "minecraft:vindicator_spawn_egg" => "minecraft:vindicator",
        "minecraft:wandering_trader_spawn_egg" => "minecraft:wandering_trader",
        "minecraft:warden_spawn_egg" => "minecraft:warden",
        "minecraft:witch_spawn_egg" => "minecraft:witch",
        "minecraft:wither_spawn_egg" => "minecraft:wither",
        "minecraft:wither_skeleton_spawn_egg" => "minecraft:wither_skeleton",
        "minecraft:wolf_spawn_egg" => "minecraft:wolf",
        "minecraft:zoglin_spawn_egg" => "minecraft:zoglin",
        "minecraft:zombie_spawn_egg" => "minecraft:zombie",
        "minecraft:zombie_horse_spawn_egg" => "minecraft:zombie_horse",
        "minecraft:zombie_villager_spawn_egg" => "minecraft:zombie_villager",
        "minecraft:zombified_piglin_spawn_egg" => "minecraft:zombified_piglin",
        "minecraft:item_frame" => "minecraft:item_frame",
        "minecraft:boat" | "minecraft:oak_boat" => "minecraft:oak_boat",
        "minecraft:oak_chest_boat" => "minecraft:oak_chest_boat",
        "minecraft:spruce_boat" => "minecraft:spruce_boat",
        "minecraft:spruce_chest_boat" => "minecraft:spruce_chest_boat",
        "minecraft:birch_boat" => "minecraft:birch_boat",
        "minecraft:birch_chest_boat" => "minecraft:birch_chest_boat",
        "minecraft:jungle_boat" => "minecraft:jungle_boat",
        "minecraft:jungle_chest_boat" => "minecraft:jungle_chest_boat",
        "minecraft:acacia_boat" => "minecraft:acacia_boat",
        "minecraft:acacia_chest_boat" => "minecraft:acacia_chest_boat",
        "minecraft:cherry_boat" => "minecraft:cherry_boat",
        "minecraft:cherry_chest_boat" => "minecraft:cherry_chest_boat",
        "minecraft:dark_oak_boat" => "minecraft:dark_oak_boat",
        "minecraft:dark_oak_chest_boat" => "minecraft:dark_oak_chest_boat",
        "minecraft:mangrove_boat" => "minecraft:mangrove_boat",
        "minecraft:mangrove_chest_boat" => "minecraft:mangrove_chest_boat",
        "minecraft:bamboo_raft" => "minecraft:bamboo_raft",
        "minecraft:bamboo_chest_raft" => "minecraft:bamboo_chest_raft",
        "minecraft:minecart" => "minecraft:minecart",
        "minecraft:chest_minecart" => "minecraft:chest_minecart",
        "minecraft:furnace_minecart" => "minecraft:furnace_minecart",
        "minecraft:tnt_minecart" => "minecraft:tnt_minecart",
        "minecraft:hopper_minecart" => "minecraft:hopper_minecart",
        _ => return None,
    })
}

pub fn add_names_hook(value: Dynamic) -> Dynamic {
    v99::add_names(value, v704::item_to_blockentity, item_to_entity)
}

pub fn register_mob(map: &mut TypeTemplates, name: &str) {
    register_simple(map, name);
}

pub fn register_throwable_projectile(map: &mut TypeTemplates, name: &str) {
    register(map, name, |s| optional_fields(vec![("inTile", BLOCK_NAME.in_schema(s))]));
}

fn register_entities(_parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = TypeTemplates::new();
    register(&mut map, "minecraft:area_effect_cloud", |s| optional_fields(vec![("Particle", PARTICLE.in_schema(s))]));
    register_mob(&mut map, "minecraft:armor_stand");
    register(&mut map, "minecraft:arrow", |s| optional_fields(vec![("inTile", BLOCK_NAME.in_schema(s))]));
    register_mob(&mut map, "minecraft:bat");
    register_mob(&mut map, "minecraft:blaze");
    register_simple(&mut map, "minecraft:boat");
    register_mob(&mut map, "minecraft:cave_spider");
    register(&mut map, "minecraft:chest_minecart", |s| {
        optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))])
    });
    register_mob(&mut map, "minecraft:chicken");
    register(&mut map, "minecraft:commandblock_minecart", |s| {
        optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s)), ("LastOutput", TEXT_COMPONENT.in_schema(s))])
    });
    register_mob(&mut map, "minecraft:cow");
    register_mob(&mut map, "minecraft:creeper");
    register(&mut map, "minecraft:donkey", |s| {
        optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s))), ("SaddleItem", ITEM_STACK.in_schema(s))])
    });
    register_simple(&mut map, "minecraft:dragon_fireball");
    register_throwable_projectile(&mut map, "minecraft:egg");
    register_mob(&mut map, "minecraft:elder_guardian");
    register_simple(&mut map, "minecraft:ender_crystal");
    register_mob(&mut map, "minecraft:ender_dragon");
    register(&mut map, "minecraft:enderman", |s| optional_fields(vec![("carried", BLOCK_NAME.in_schema(s))]));
    register_mob(&mut map, "minecraft:endermite");
    register_throwable_projectile(&mut map, "minecraft:ender_pearl");
    register_simple(&mut map, "minecraft:eye_of_ender_signal");
    register(&mut map, "minecraft:falling_block", |s| {
        optional_fields(vec![("Block", BLOCK_NAME.in_schema(s)), ("TileEntityData", BLOCK_ENTITY.in_schema(s))])
    });
    register_throwable_projectile(&mut map, "minecraft:fireball");
    register(&mut map, "minecraft:fireworks_rocket", |s| optional_fields(vec![("FireworksItem", ITEM_STACK.in_schema(s))]));
    register(&mut map, "minecraft:furnace_minecart", |s| optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s))]));
    register_mob(&mut map, "minecraft:ghast");
    register_mob(&mut map, "minecraft:giant");
    register_mob(&mut map, "minecraft:guardian");
    register(&mut map, "minecraft:hopper_minecart", |s| {
        optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))])
    });
    register(&mut map, "minecraft:horse", |s| optional_fields(vec![("ArmorItem", ITEM_STACK.in_schema(s)), ("SaddleItem", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:husk");
    register(&mut map, "minecraft:item", |s| optional_fields(vec![("Item", ITEM_STACK.in_schema(s))]));
    register(&mut map, "minecraft:item_frame", |s| optional_fields(vec![("Item", ITEM_STACK.in_schema(s))]));
    register_simple(&mut map, "minecraft:leash_knot");
    register_mob(&mut map, "minecraft:magma_cube");
    register(&mut map, "minecraft:minecart", |s| optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s))]));
    register_mob(&mut map, "minecraft:mooshroom");
    register(&mut map, "minecraft:mule", |s| optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s))), ("SaddleItem", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:ocelot");
    register_simple(&mut map, "minecraft:painting");
    register_mob(&mut map, "minecraft:parrot");
    register_mob(&mut map, "minecraft:pig");
    register_mob(&mut map, "minecraft:polar_bear");
    register(&mut map, "minecraft:potion", |s| optional_fields(vec![("Potion", ITEM_STACK.in_schema(s)), ("inTile", BLOCK_NAME.in_schema(s))]));
    register_mob(&mut map, "minecraft:rabbit");
    register_mob(&mut map, "minecraft:sheep");
    register_mob(&mut map, "minecraft:shulker");
    register_simple(&mut map, "minecraft:shulker_bullet");
    register_mob(&mut map, "minecraft:silverfish");
    register_mob(&mut map, "minecraft:skeleton");
    register(&mut map, "minecraft:skeleton_horse", |s| optional_fields(vec![("SaddleItem", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:slime");
    register_throwable_projectile(&mut map, "minecraft:small_fireball");
    register_throwable_projectile(&mut map, "minecraft:snowball");
    register_mob(&mut map, "minecraft:snowman");
    register(&mut map, "minecraft:spawner_minecart", |s| {
        optional_fields_with_rest(vec![("DisplayTile", BLOCK_NAME.in_schema(s))], UNTAGGED_SPAWNER.in_schema(s))
    });
    register(&mut map, "minecraft:spectral_arrow", |s| optional_fields(vec![("inTile", BLOCK_NAME.in_schema(s))]));
    register_mob(&mut map, "minecraft:spider");
    register_mob(&mut map, "minecraft:squid");
    register_mob(&mut map, "minecraft:stray");
    register_simple(&mut map, "minecraft:tnt");
    register(&mut map, "minecraft:tnt_minecart", |s| optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s))]));
    register(&mut map, "minecraft:villager", |s| {
        optional_fields(vec![
            ("Inventory", list(ITEM_STACK.in_schema(s))),
            ("Offers", optional_fields(vec![("Recipes", list(VILLAGER_TRADE.in_schema(s)))])),
        ])
    });
    register_mob(&mut map, "minecraft:villager_golem");
    register_mob(&mut map, "minecraft:witch");
    register_mob(&mut map, "minecraft:wither");
    register_mob(&mut map, "minecraft:wither_skeleton");
    register_throwable_projectile(&mut map, "minecraft:wither_skull");
    register_mob(&mut map, "minecraft:wolf");
    register_throwable_projectile(&mut map, "minecraft:xp_bottle");
    register_simple(&mut map, "minecraft:xp_orb");
    register_mob(&mut map, "minecraft:zombie");
    register(&mut map, "minecraft:zombie_horse", |s| optional_fields(vec![("SaddleItem", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:zombie_pigman");
    register(&mut map, "minecraft:zombie_villager", |s| {
        optional_fields(vec![("Offers", optional_fields(vec![("Recipes", list(VILLAGER_TRADE.in_schema(s)))]))])
    });
    register_simple(&mut map, "minecraft:evocation_fangs");
    register_mob(&mut map, "minecraft:evocation_illager");
    register_mob(&mut map, "minecraft:illusion_illager");
    register(&mut map, "minecraft:llama", |s| {
        optional_fields(vec![
            ("Items", list(ITEM_STACK.in_schema(s))),
            ("SaddleItem", ITEM_STACK.in_schema(s)),
            ("DecorItem", ITEM_STACK.in_schema(s)),
        ])
    });
    register_simple(&mut map, "minecraft:llama_spit");
    register_mob(&mut map, "minecraft:vex");
    register_mob(&mut map, "minecraft:vindication_illager");
    map
}

fn register_types(parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    parent.expect("a parent schema").register_types(registry, entity_types, block_entity_types);
    let entities = entity_types.clone();
    registry.register_type(true, ENTITY, move |s| {
        and(
            ENTITY_EQUIPMENT.in_schema(s),
            optional_fields_with_rest(vec![("CustomName", string())], tagged_choice_lazy("id", ConstType::NamespacedString, entities.clone())),
        )
    });
    registry.register_type(true, ITEM_STACK, |s| {
        hook(optional_fields(vec![("id", ITEM_NAME.in_schema(s)), ("tag", v99::item_stack_tag(s))]), add_names_hook, identity_hook)
    });
}
