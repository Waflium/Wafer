use std::sync::Arc;

use crate::datafixers::dsl::{ConstType, TypeTemplates, and, and_all, compound_list, compound_list_with_key, const_type, field, fields, hook, identity_hook, int_type, list, optional, optional_fields, optional_fields_with_rest, or, remainder, string, tagged_choice_lazy};
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::{v1451_6, v1458, v705, v99};

pub static V1460: SchemaClass = SchemaClass {
    namespaced: true,
    namespaced_block_entity_choices: false,
    register_types: Some(register_types),
    register_entities: Some(register_entities),
    register_block_entities: Some(register_block_entities),
};

pub fn register_mob(map: &mut TypeTemplates, name: &str) {
    register_simple(map, name);
}

pub fn register_inventory(map: &mut TypeTemplates, name: &str) {
    register(map, name, |s| v1458::nameable_inventory(s));
}

fn register_entities(_parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = TypeTemplates::new();
    register(&mut map, "minecraft:area_effect_cloud", |s| optional_fields(vec![("Particle", PARTICLE.in_schema(s))]));
    register_mob(&mut map, "minecraft:armor_stand");
    register(&mut map, "minecraft:arrow", |s| optional_fields(vec![("inBlockState", BLOCK_STATE.in_schema(s))]));
    register_mob(&mut map, "minecraft:bat");
    register_mob(&mut map, "minecraft:blaze");
    register_simple(&mut map, "minecraft:boat");
    register_mob(&mut map, "minecraft:cave_spider");
    register(&mut map, "minecraft:chest_minecart", |s| {
        optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))])
    });
    register_mob(&mut map, "minecraft:chicken");
    register(&mut map, "minecraft:commandblock_minecart", |s| {
        optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s)), ("LastOutput", TEXT_COMPONENT.in_schema(s))])
    });
    register_mob(&mut map, "minecraft:cow");
    register_mob(&mut map, "minecraft:creeper");
    register(&mut map, "minecraft:donkey", |s| {
        optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s))), ("SaddleItem", ITEM_STACK.in_schema(s))])
    });
    register_simple(&mut map, "minecraft:dragon_fireball");
    register_simple(&mut map, "minecraft:egg");
    register_mob(&mut map, "minecraft:elder_guardian");
    register_simple(&mut map, "minecraft:ender_crystal");
    register_mob(&mut map, "minecraft:ender_dragon");
    register(&mut map, "minecraft:enderman", |s| optional_fields(vec![("carriedBlockState", BLOCK_STATE.in_schema(s))]));
    register_mob(&mut map, "minecraft:endermite");
    register_simple(&mut map, "minecraft:ender_pearl");
    register_simple(&mut map, "minecraft:evocation_fangs");
    register_mob(&mut map, "minecraft:evocation_illager");
    register_simple(&mut map, "minecraft:eye_of_ender_signal");
    register(&mut map, "minecraft:falling_block", |s| {
        optional_fields(vec![("BlockState", BLOCK_STATE.in_schema(s)), ("TileEntityData", BLOCK_ENTITY.in_schema(s))])
    });
    register_simple(&mut map, "minecraft:fireball");
    register(&mut map, "minecraft:fireworks_rocket", |s| optional_fields(vec![("FireworksItem", ITEM_STACK.in_schema(s))]));
    register(&mut map, "minecraft:furnace_minecart", |s| optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s))]));
    register_mob(&mut map, "minecraft:ghast");
    register_mob(&mut map, "minecraft:giant");
    register_mob(&mut map, "minecraft:guardian");
    register(&mut map, "minecraft:hopper_minecart", |s| {
        optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))])
    });
    register(&mut map, "minecraft:horse", |s| optional_fields(vec![("ArmorItem", ITEM_STACK.in_schema(s)), ("SaddleItem", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:husk");
    register_mob(&mut map, "minecraft:illusion_illager");
    register(&mut map, "minecraft:item", |s| optional_fields(vec![("Item", ITEM_STACK.in_schema(s))]));
    register(&mut map, "minecraft:item_frame", |s| optional_fields(vec![("Item", ITEM_STACK.in_schema(s))]));
    register_simple(&mut map, "minecraft:leash_knot");
    register(&mut map, "minecraft:llama", |s| {
        optional_fields(vec![
            ("Items", list(ITEM_STACK.in_schema(s))),
            ("SaddleItem", ITEM_STACK.in_schema(s)),
            ("DecorItem", ITEM_STACK.in_schema(s)),
        ])
    });
    register_simple(&mut map, "minecraft:llama_spit");
    register_mob(&mut map, "minecraft:magma_cube");
    register(&mut map, "minecraft:minecart", |s| optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s))]));
    register_mob(&mut map, "minecraft:mooshroom");
    register(&mut map, "minecraft:mule", |s| optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s))), ("SaddleItem", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:ocelot");
    register_simple(&mut map, "minecraft:painting");
    register_mob(&mut map, "minecraft:parrot");
    register_mob(&mut map, "minecraft:pig");
    register_mob(&mut map, "minecraft:polar_bear");
    register(&mut map, "minecraft:potion", |s| optional_fields(vec![("Potion", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:rabbit");
    register_mob(&mut map, "minecraft:sheep");
    register_mob(&mut map, "minecraft:shulker");
    register_simple(&mut map, "minecraft:shulker_bullet");
    register_mob(&mut map, "minecraft:silverfish");
    register_mob(&mut map, "minecraft:skeleton");
    register(&mut map, "minecraft:skeleton_horse", |s| optional_fields(vec![("SaddleItem", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:slime");
    register_simple(&mut map, "minecraft:small_fireball");
    register_simple(&mut map, "minecraft:snowball");
    register_mob(&mut map, "minecraft:snowman");
    register(&mut map, "minecraft:spawner_minecart", |s| {
        optional_fields_with_rest(vec![("DisplayState", BLOCK_STATE.in_schema(s))], UNTAGGED_SPAWNER.in_schema(s))
    });
    register(&mut map, "minecraft:spectral_arrow", |s| optional_fields(vec![("inBlockState", BLOCK_STATE.in_schema(s))]));
    register_mob(&mut map, "minecraft:spider");
    register_mob(&mut map, "minecraft:squid");
    register_mob(&mut map, "minecraft:stray");
    register_simple(&mut map, "minecraft:tnt");
    register(&mut map, "minecraft:tnt_minecart", |s| optional_fields(vec![("DisplayState", BLOCK_STATE.in_schema(s))]));
    register_mob(&mut map, "minecraft:vex");
    register(&mut map, "minecraft:villager", |s| {
        optional_fields(vec![
            ("Inventory", list(ITEM_STACK.in_schema(s))),
            ("Offers", optional_fields(vec![("Recipes", list(VILLAGER_TRADE.in_schema(s)))])),
        ])
    });
    register_mob(&mut map, "minecraft:villager_golem");
    register_mob(&mut map, "minecraft:vindication_illager");
    register_mob(&mut map, "minecraft:witch");
    register_mob(&mut map, "minecraft:wither");
    register_mob(&mut map, "minecraft:wither_skeleton");
    register_simple(&mut map, "minecraft:wither_skull");
    register_mob(&mut map, "minecraft:wolf");
    register_simple(&mut map, "minecraft:xp_bottle");
    register_simple(&mut map, "minecraft:xp_orb");
    register_mob(&mut map, "minecraft:zombie");
    register(&mut map, "minecraft:zombie_horse", |s| optional_fields(vec![("SaddleItem", ITEM_STACK.in_schema(s))]));
    register_mob(&mut map, "minecraft:zombie_pigman");
    register(&mut map, "minecraft:zombie_villager", |s| {
        optional_fields(vec![("Offers", optional_fields(vec![("Recipes", list(VILLAGER_TRADE.in_schema(s)))]))])
    });
    map
}

fn register_block_entities(_parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = TypeTemplates::new();
    register_inventory(&mut map, "minecraft:furnace");
    register_inventory(&mut map, "minecraft:chest");
    register_inventory(&mut map, "minecraft:trapped_chest");
    register_simple(&mut map, "minecraft:ender_chest");
    register(&mut map, "minecraft:jukebox", |s| optional_fields(vec![("RecordItem", ITEM_STACK.in_schema(s))]));
    register_inventory(&mut map, "minecraft:dispenser");
    register_inventory(&mut map, "minecraft:dropper");
    register(&mut map, "minecraft:sign", |s| v99::sign(s));
    register(&mut map, "minecraft:mob_spawner", |s| UNTAGGED_SPAWNER.in_schema(s));
    register(&mut map, "minecraft:piston", |s| optional_fields(vec![("blockState", BLOCK_STATE.in_schema(s))]));
    register_inventory(&mut map, "minecraft:brewing_stand");
    register(&mut map, "minecraft:enchanting_table", |s| v1458::nameable(s));
    register_simple(&mut map, "minecraft:end_portal");
    register(&mut map, "minecraft:beacon", |s| v1458::nameable(s));
    register(&mut map, "minecraft:skull", |s| optional_fields(vec![("custom_name", TEXT_COMPONENT.in_schema(s))]));
    register_simple(&mut map, "minecraft:daylight_detector");
    register_inventory(&mut map, "minecraft:hopper");
    register_simple(&mut map, "minecraft:comparator");
    register(&mut map, "minecraft:banner", |s| v1458::nameable(s));
    register_simple(&mut map, "minecraft:structure_block");
    register_simple(&mut map, "minecraft:end_gateway");
    register(&mut map, "minecraft:command_block", |s| optional_fields(vec![("LastOutput", TEXT_COMPONENT.in_schema(s))]));
    register_inventory(&mut map, "minecraft:shulker_box");
    register_simple(&mut map, "minecraft:bed");
    map
}

fn register_types(_parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    registry.register_type(false, LEVEL, |s| {
        optional_fields_with_rest(vec![
            ("CustomBossEvents", compound_list(optional_fields(vec![("Name", TEXT_COMPONENT.in_schema(s))]))),
        ], LIGHTWEIGHT_LEVEL.in_schema(s))
    });
    registry.register_type(false, LIGHTWEIGHT_LEVEL, |_| remainder());
    registry.register_type(false, RECIPE, |_| const_type(ConstType::NamespacedString));
    registry.register_type(false, PLAYER, |s| {
        optional_fields(vec![
            ("RootVehicle", optional_fields(vec![("Entity", ENTITY_TREE.in_schema(s))])),
            ("ender_pearls", list(ENTITY_TREE.in_schema(s))),
            ("Inventory", list(ITEM_STACK.in_schema(s))),
            ("EnderItems", list(ITEM_STACK.in_schema(s))),
            ("ShoulderEntityLeft", ENTITY_TREE.in_schema(s)),
            ("ShoulderEntityRight", ENTITY_TREE.in_schema(s)),
            ("recipeBook", optional_fields(vec![("recipes", list(RECIPE.in_schema(s))), ("toBeDisplayed", list(RECIPE.in_schema(s)))])),
        ])
    });
    registry.register_type(false, CHUNK, |s| {
        fields(vec![
            ("Level", optional_fields(vec![
                ("Entities", list(ENTITY_TREE.in_schema(s))),
                ("TileEntities", list(or(BLOCK_ENTITY.in_schema(s), remainder()))),
                ("TileTicks", list(fields(vec![("i", BLOCK_NAME.in_schema(s))]))),
                ("Sections", list(optional_fields(vec![("Palette", list(BLOCK_STATE.in_schema(s)))]))),
            ])),
        ])
    });
    let block_entities = block_entity_types.clone();
    registry.register_type(true, BLOCK_ENTITY, move |s| {
        optional_fields_with_rest(
            vec![("components", DATA_COMPONENTS.in_schema(s))],
            tagged_choice_lazy("id", ConstType::NamespacedString, block_entities.clone()),
        )
    });
    registry.register_type(true, ENTITY_TREE, |s| {
        optional_fields_with_rest(vec![("Passengers", list(ENTITY_TREE.in_schema(s)))], ENTITY.in_schema(s))
    });
    let entities = entity_types.clone();
    registry.register_type(true, ENTITY, move |s| {
        and(
            ENTITY_EQUIPMENT.in_schema(s),
            optional_fields_with_rest(
                vec![("CustomName", TEXT_COMPONENT.in_schema(s))],
                tagged_choice_lazy("id", ConstType::NamespacedString, entities.clone()),
            ),
        )
    });
    registry.register_type(true, ITEM_STACK, |s| {
        hook(optional_fields(vec![("id", ITEM_NAME.in_schema(s)), ("tag", v99::item_stack_tag(s))]), v705::add_names_hook, identity_hook)
    });
    registry.register_type(false, HOTBAR, |s| compound_list(list(ITEM_STACK.in_schema(s))));
    registry.register_type(false, OPTIONS, |_| remainder());
    registry.register_type(false, STRUCTURE, |s| {
        optional_fields(vec![
            ("entities", list(optional_fields(vec![("nbt", ENTITY_TREE.in_schema(s))]))),
            ("blocks", list(optional_fields(vec![("nbt", BLOCK_ENTITY.in_schema(s))]))),
            ("palette", list(BLOCK_STATE.in_schema(s))),
            ("palettes", list(list(BLOCK_STATE.in_schema(s)))),
        ])
    });
    registry.register_type(false, BLOCK_NAME, |_| const_type(ConstType::NamespacedString));
    registry.register_type(false, ITEM_NAME, |_| const_type(ConstType::NamespacedString));
    registry.register_type(false, BLOCK_STATE, |_| remainder());
    registry.register_type(false, FLAT_BLOCK_STATE, |_| remainder());
    registry.register_type(false, STATS, |s| {
        optional_fields(vec![
            ("stats", optional_fields(vec![
                ("minecraft:mined", compound_list_with_key(BLOCK_NAME.in_schema(s), int_type())),
                ("minecraft:crafted", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:used", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:broken", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:picked_up", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:dropped", compound_list_with_key(ITEM_NAME.in_schema(s), int_type())),
                ("minecraft:killed", compound_list_with_key(ENTITY_NAME.in_schema(s), int_type())),
                ("minecraft:killed_by", compound_list_with_key(ENTITY_NAME.in_schema(s), int_type())),
                ("minecraft:custom", compound_list_with_key(const_type(ConstType::NamespacedString), int_type())),
            ])),
        ])
    });
    registry.register_type(false, SAVED_DATA_COMMAND_STORAGE, |_| remainder());
    registry.register_type(false, SAVED_DATA_CUSTOM_BOSS_EVENTS, |s| {
        optional_fields(vec![("data", compound_list(optional_fields(vec![("Name", TEXT_COMPONENT.in_schema(s))])))])
    });
    registry.register_type(false, SAVED_DATA_ENDER_DRAGON_FIGHT, |_| remainder());
    registry.register_type(false, SAVED_DATA_GAME_RULES, |_| remainder());
    registry.register_type(false, SAVED_DATA_TICKETS, |_| remainder());
    registry.register_type(false, SAVED_DATA_MAP_DATA, |s| {
        optional_fields(vec![("data", optional_fields(vec![("banners", list(optional_fields(vec![("Name", TEXT_COMPONENT.in_schema(s))])))]))])
    });
    registry.register_type(false, SAVED_DATA_MAP_INDEX, |_| remainder());
    registry.register_type(false, SAVED_DATA_RAIDS, |_| remainder());
    registry.register_type(false, SAVED_DATA_RANDOM_SEQUENCES, |_| remainder());
    registry.register_type(false, SAVED_DATA_SCHEDULED_EVENTS, |_| remainder());
    registry.register_type(false, SAVED_DATA_SCOREBOARD, |s| {
        optional_fields(vec![
            ("data", optional_fields(vec![
                ("Objectives", list(OBJECTIVE.in_schema(s))),
                ("Teams", list(TEAM.in_schema(s))),
                ("PlayerScores", list(optional_fields(vec![("display", TEXT_COMPONENT.in_schema(s))]))),
            ])),
        ])
    });
    registry.register_type(false, SAVED_DATA_STOPWATCHES, |_| remainder());
    registry.register_type(false, SAVED_DATA_STRUCTURE_FEATURE_INDICES, |s| {
        optional_fields(vec![("data", optional_fields(vec![("Features", compound_list(STRUCTURE_FEATURE.in_schema(s)))]))])
    });
    registry.register_type(false, SAVED_DATA_WANDERING_TRADER, |_| remainder());
    registry.register_type(false, SAVED_DATA_WEATHER, |_| remainder());
    registry.register_type(false, SAVED_DATA_WORLD_BORDER, |_| remainder());
    registry.register_type(false, SAVED_DATA_WORLD_CLOCKS, |_| remainder());
    registry.register_type(false, SAVED_DATA_WORLD_GEN_SETTINGS, |s| fields(vec![("data", WORLD_GEN_SETTINGS.in_schema(s))]));
    registry.register_type(false, DEBUG_PROFILE, |_| remainder());
    registry.register_type(false, STRUCTURE_FEATURE, |_| remainder());
    let criterion_types = Arc::new(v1451_6::create_criterion_types());
    registry.register_type(false, OBJECTIVE, move |s| {
        hook(
            optional_fields(vec![
                ("CriteriaType", tagged_choice_lazy("type", ConstType::String, criterion_types.clone())),
                ("DisplayName", TEXT_COMPONENT.in_schema(s)),
            ]),
            v1451_6::unpack_objective_id,
            v1451_6::repack_objective_id,
        )
    });
    registry.register_type(false, TEAM, |s| {
        optional_fields(vec![
            ("MemberNamePrefix", TEXT_COMPONENT.in_schema(s)),
            ("MemberNameSuffix", TEXT_COMPONENT.in_schema(s)),
            ("DisplayName", TEXT_COMPONENT.in_schema(s)),
        ])
    });
    registry.register_type(true, UNTAGGED_SPAWNER, |s| {
        optional_fields(vec![("SpawnPotentials", list(fields(vec![("Entity", ENTITY_TREE.in_schema(s))]))), ("SpawnData", ENTITY_TREE.in_schema(s))])
    });
    registry.register_type(false, ADVANCEMENTS, |s| {
        optional_fields(vec![
            ("minecraft:adventure/adventuring_time", optional_fields(vec![("criteria", compound_list_with_key(BIOME.in_schema(s), string()))])),
            ("minecraft:adventure/kill_a_mob", optional_fields(vec![("criteria", compound_list_with_key(ENTITY_NAME.in_schema(s), string()))])),
            ("minecraft:adventure/kill_all_mobs", optional_fields(vec![("criteria", compound_list_with_key(ENTITY_NAME.in_schema(s), string()))])),
            ("minecraft:husbandry/bred_all_animals", optional_fields(vec![("criteria", compound_list_with_key(ENTITY_NAME.in_schema(s), string()))])),
        ])
    });
    registry.register_type(false, BIOME, |_| const_type(ConstType::NamespacedString));
    registry.register_type(false, ENTITY_NAME, |_| const_type(ConstType::NamespacedString));
    registry.register_type(false, POI_CHUNK, |_| remainder());
    registry.register_type(false, WORLD_GEN_SETTINGS, |_| remainder());
    registry.register_type(false, ENTITY_CHUNK, |s| optional_fields(vec![("Entities", list(ENTITY_TREE.in_schema(s)))]));
    registry.register_type(true, DATA_COMPONENTS, |_| remainder());
    registry.register_type(true, VILLAGER_TRADE, |s| {
        optional_fields(vec![("buy", ITEM_STACK.in_schema(s)), ("buyB", ITEM_STACK.in_schema(s)), ("sell", ITEM_STACK.in_schema(s))])
    });
    registry.register_type(true, PARTICLE, |_| string());
    registry.register_type(true, TEXT_COMPONENT, |_| string());
    registry.register_type(true, ENTITY_EQUIPMENT, |s| {
        and_all(vec![
            optional(field("ArmorItems", list(ITEM_STACK.in_schema(s)))),
            optional(field("HandItems", list(ITEM_STACK.in_schema(s)))),
            optional(field("body_armor_item", ITEM_STACK.in_schema(s))),
            optional(field("saddle", ITEM_STACK.in_schema(s))),
        ])
    });
}
