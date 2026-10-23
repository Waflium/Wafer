use std::sync::Arc;

use crate::datafixers::dsl::{self, ConstType, TypeTemplate, TypeTemplates, compound_list, const_type, field, int_type, list, optional, optional_fields, optional_fields_with_rest, or, remainder, string};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::{Registry, Schema, SchemaClass, register, register_simple};
use crate::util::datafix::fixes::references::*;
use crate::util::datafix::schemas::namespaced_schema::ensure_namespaced;

pub static V99: SchemaClass = SchemaClass {
    namespaced: false,
    register_types: Some(register_types),
    register_entities: Some(register_entities),
    register_block_entities: Some(register_block_entities),
};

fn item_to_block_entity(item: &str) -> Option<&'static str> {
    Some(match item {
        "minecraft:furnace" | "minecraft:lit_furnace" => "Furnace",
        "minecraft:chest" | "minecraft:trapped_chest" => "Chest",
        "minecraft:ender_chest" => "EnderChest",
        "minecraft:jukebox" => "RecordPlayer",
        "minecraft:dispenser" => "Trap",
        "minecraft:dropper" => "Dropper",
        "minecraft:sign" | "minecraft:standing_sign" | "minecraft:wall_sign" => "Sign",
        "minecraft:mob_spawner" => "MobSpawner",
        "minecraft:noteblock" => "Music",
        "minecraft:brewing_stand" => "Cauldron",
        "minecraft:enhanting_table" => "EnchantTable",
        "minecraft:command_block" | "minecraft:repeating_command_block" | "minecraft:chain_command_block" => "CommandBlock",
        "minecraft:beacon" => "Beacon",
        "minecraft:skull" => "Skull",
        "minecraft:daylight_detector" | "minecraft:daylight_detector_inverted" => "DLDetector",
        "minecraft:hopper" => "Hopper",
        "minecraft:banner" | "minecraft:wall_banner" | "minecraft:standing_banner" | "minecraft:shield" => "Banner",
        "minecraft:flower_pot" => "FlowerPot",
        "minecraft:piston_head" => "Piston",
        "minecraft:unpowered_comparator" | "minecraft:powered_comparator" => "Comparator",
        "minecraft:structure_block" => "Structure",
        "minecraft:end_portal" => "Airportal",
        "minecraft:end_gateway" => "EndGateway",
        _ => return None,
    })
}

pub fn item_to_entity(item: &str) -> Option<&'static str> {
    match item {
        "minecraft:armor_stand" => Some("ArmorStand"),
        "minecraft:painting" => Some("Painting"),
        _ => None,
    }
}

pub fn add_names_hook(value: Dynamic) -> Dynamic {
    add_names(value, item_to_block_entity, item_to_entity)
}

pub fn register_throwable_projectile(map: &mut TypeTemplates, name: &str) {
    register(map, name, |s| optional_fields(vec![("inTile", BLOCK_NAME.in_schema(s))]));
}

pub fn register_minecart(map: &mut TypeTemplates, name: &str) {
    register(map, name, |s| optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s))]));
}

pub fn register_inventory(map: &mut TypeTemplates, name: &str) {
    register(map, name, |s| optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s)))]));
}

fn register_entities(_parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = TypeTemplates::new();
    register(&mut map, "Item", |s| optional_fields(vec![("Item", ITEM_STACK.in_schema(s))]));
    register_simple(&mut map, "XPOrb");
    register_throwable_projectile(&mut map, "ThrownEgg");
    register_simple(&mut map, "LeashKnot");
    register_simple(&mut map, "Painting");
    register(&mut map, "Arrow", |s| optional_fields(vec![("inTile", BLOCK_NAME.in_schema(s))]));
    register(&mut map, "TippedArrow", |s| optional_fields(vec![("inTile", BLOCK_NAME.in_schema(s))]));
    register(&mut map, "SpectralArrow", |s| optional_fields(vec![("inTile", BLOCK_NAME.in_schema(s))]));
    register_throwable_projectile(&mut map, "Snowball");
    register_throwable_projectile(&mut map, "Fireball");
    register_throwable_projectile(&mut map, "SmallFireball");
    register_throwable_projectile(&mut map, "ThrownEnderpearl");
    register_simple(&mut map, "EyeOfEnderSignal");
    register(&mut map, "ThrownPotion", |s| optional_fields(vec![("inTile", BLOCK_NAME.in_schema(s)), ("Potion", ITEM_STACK.in_schema(s))]));
    register_throwable_projectile(&mut map, "ThrownExpBottle");
    register(&mut map, "ItemFrame", |s| optional_fields(vec![("Item", ITEM_STACK.in_schema(s))]));
    register_throwable_projectile(&mut map, "WitherSkull");
    register_simple(&mut map, "PrimedTnt");
    register(&mut map, "FallingSand", |s| optional_fields(vec![("Block", BLOCK_NAME.in_schema(s)), ("TileEntityData", BLOCK_ENTITY.in_schema(s))]));
    register(&mut map, "FireworksRocketEntity", |s| optional_fields(vec![("FireworksItem", ITEM_STACK.in_schema(s))]));
    register_simple(&mut map, "Boat");
    register(&mut map, "Minecart", |s| optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))]));
    register_minecart(&mut map, "MinecartRideable");
    register(&mut map, "MinecartChest", |s| optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))]));
    register_minecart(&mut map, "MinecartFurnace");
    register_minecart(&mut map, "MinecartTNT");
    register(&mut map, "MinecartSpawner", |s| optional_fields_with_rest(vec![("DisplayTile", BLOCK_NAME.in_schema(s))], UNTAGGED_SPAWNER.in_schema(s)));
    register(&mut map, "MinecartHopper", |s| optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s)), ("Items", list(ITEM_STACK.in_schema(s)))]));
    register(&mut map, "MinecartCommandBlock", |s| optional_fields(vec![("DisplayTile", BLOCK_NAME.in_schema(s)), ("LastOutput", TEXT_COMPONENT.in_schema(s))]));
    for name in [
        "ArmorStand", "Creeper", "Skeleton", "Spider", "Giant", "Zombie", "Slime", "Ghast", "PigZombie",
    ] {
        register_simple(&mut map, name);
    }
    register(&mut map, "Enderman", |s| optional_fields(vec![("carried", BLOCK_NAME.in_schema(s))]));
    for name in [
        "CaveSpider", "Silverfish", "Blaze", "LavaSlime", "EnderDragon", "WitherBoss", "Bat", "Witch", "Endermite", "Guardian", "Pig", "Sheep", "Cow", "Chicken",
        "Squid", "Wolf", "MushroomCow", "SnowMan", "Ozelot", "VillagerGolem",
    ] {
        register_simple(&mut map, name);
    }
    register(&mut map, "EntityHorse", |s| {
        optional_fields(vec![("Items", list(ITEM_STACK.in_schema(s))), ("ArmorItem", ITEM_STACK.in_schema(s)), ("SaddleItem", ITEM_STACK.in_schema(s))])
    });
    register_simple(&mut map, "Rabbit");
    register(&mut map, "Villager", |s| {
        optional_fields(vec![("Inventory", list(ITEM_STACK.in_schema(s))), ("Offers", optional_fields(vec![("Recipes", list(VILLAGER_TRADE.in_schema(s)))]))])
    });
    register_simple(&mut map, "EnderCrystal");
    register(&mut map, "AreaEffectCloud", |s| optional_fields(vec![("Particle", PARTICLE.in_schema(s))]));
    register_simple(&mut map, "ShulkerBullet");
    register_simple(&mut map, "DragonFireball");
    register_simple(&mut map, "Shulker");
    map
}

fn register_block_entities(_parent: Option<&Arc<Schema>>) -> TypeTemplates {
    let mut map = TypeTemplates::new();
    register_inventory(&mut map, "Furnace");
    register_inventory(&mut map, "Chest");
    register_simple(&mut map, "EnderChest");
    register(&mut map, "RecordPlayer", |s| optional_fields(vec![("RecordItem", ITEM_STACK.in_schema(s))]));
    register_inventory(&mut map, "Trap");
    register_inventory(&mut map, "Dropper");
    register(&mut map, "Sign", sign);
    register(&mut map, "MobSpawner", |s| UNTAGGED_SPAWNER.in_schema(s));
    register_simple(&mut map, "Music");
    register_simple(&mut map, "Piston");
    register_inventory(&mut map, "Cauldron");
    register_simple(&mut map, "EnchantTable");
    register_simple(&mut map, "Airportal");
    register(&mut map, "Control", |s| optional_fields(vec![("LastOutput", TEXT_COMPONENT.in_schema(s))]));
    register_simple(&mut map, "Beacon");
    register(&mut map, "Skull", |s| optional_fields(vec![("custom_name", TEXT_COMPONENT.in_schema(s))]));
    register_simple(&mut map, "DLDetector");
    register_inventory(&mut map, "Hopper");
    register_simple(&mut map, "Comparator");
    register(&mut map, "FlowerPot", |s| optional_fields(vec![("Item", or(int_type(), ITEM_NAME.in_schema(s)))]));
    register(&mut map, "Banner", |s| optional_fields(vec![("CustomName", TEXT_COMPONENT.in_schema(s))]));
    register_simple(&mut map, "Structure");
    register_simple(&mut map, "EndGateway");
    map
}

pub fn sign(schema: &Schema) -> TypeTemplate {
    optional_fields(
        ["Text1", "Text2", "Text3", "Text4", "FilteredText1", "FilteredText2", "FilteredText3", "FilteredText4"]
            .into_iter()
            .map(|name| (name, TEXT_COMPONENT.in_schema(schema)))
            .collect(),
    )
}

fn register_types(_parent: Option<&Arc<Schema>>, registry: &mut Registry, entity_types: &Arc<TypeTemplates>, block_entity_types: &Arc<TypeTemplates>) {
    registry.register_type(false, LEVEL, |s| {
        optional_fields_with_rest(vec![("CustomBossEvents", compound_list(optional_fields(vec![("Name", TEXT_COMPONENT.in_schema(s))])))], LIGHTWEIGHT_LEVEL.in_schema(s))
    });
    registry.register_type(false, LIGHTWEIGHT_LEVEL, |_| remainder());
    registry.register_type(false, PLAYER, |s| optional_fields(vec![("Inventory", list(ITEM_STACK.in_schema(s))), ("EnderItems", list(ITEM_STACK.in_schema(s)))]));
    registry.register_type(false, CHUNK, |s| {
        dsl::fields(vec![(
            "Level",
            optional_fields(vec![
                ("Entities", list(ENTITY_TREE.in_schema(s))),
                ("TileEntities", list(or(BLOCK_ENTITY.in_schema(s), remainder()))),
                ("TileTicks", list(dsl::fields(vec![("i", BLOCK_NAME.in_schema(s))]))),
            ]),
        )])
    });
    let block_entities = block_entity_types.clone();
    registry.register_type(true, BLOCK_ENTITY, move |s| {
        optional_fields_with_rest(vec![("components", DATA_COMPONENTS.in_schema(s))], dsl::tagged_choice_lazy("id", ConstType::String, block_entities.clone()))
    });
    registry.register_type(true, ENTITY_TREE, |s| optional_fields_with_rest(vec![("Riding", ENTITY_TREE.in_schema(s))], ENTITY.in_schema(s)));
    registry.register_type(false, ENTITY_NAME, |_| const_type(ConstType::NamespacedString));
    let entities = entity_types.clone();
    registry.register_type(true, ENTITY, move |s| {
        dsl::and(
            ENTITY_EQUIPMENT.in_schema(s),
            optional_fields_with_rest(vec![("CustomName", string())], dsl::tagged_choice_lazy("id", ConstType::String, entities.clone())),
        )
    });
    registry.register_type(true, ITEM_STACK, |s| {
        dsl::hook(
            optional_fields(vec![("id", or(int_type(), ITEM_NAME.in_schema(s))), ("tag", item_stack_tag(s))]),
            add_names_hook,
            dsl::identity_hook,
        )
    });
    registry.register_type(false, OPTIONS, |_| remainder());
    registry.register_type(false, BLOCK_NAME, |_| or(int_type(), const_type(ConstType::NamespacedString)));
    registry.register_type(false, ITEM_NAME, |_| const_type(ConstType::NamespacedString));
    registry.register_type(false, STATS, |_| remainder());
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
        optional_fields(vec![(
            "data",
            optional_fields(vec![
                ("Objectives", list(OBJECTIVE.in_schema(s))),
                ("Teams", list(TEAM.in_schema(s))),
                ("PlayerScores", list(optional_fields(vec![("display", TEXT_COMPONENT.in_schema(s))]))),
            ]),
        )])
    });
    registry.register_type(false, SAVED_DATA_STOPWATCHES, |_| remainder());
    registry.register_type(false, SAVED_DATA_STRUCTURE_FEATURE_INDICES, |s| {
        optional_fields(vec![("data", optional_fields(vec![("Features", compound_list(STRUCTURE_FEATURE.in_schema(s)))]))])
    });
    registry.register_type(false, SAVED_DATA_WANDERING_TRADER, |_| remainder());
    registry.register_type(false, SAVED_DATA_WEATHER, |_| remainder());
    registry.register_type(false, SAVED_DATA_WORLD_BORDER, |_| remainder());
    registry.register_type(false, SAVED_DATA_WORLD_CLOCKS, |_| remainder());
    registry.register_type(false, SAVED_DATA_WORLD_GEN_SETTINGS, |s| dsl::fields(vec![("data", WORLD_GEN_SETTINGS.in_schema(s))]));
    registry.register_type(false, DEBUG_PROFILE, |_| remainder());
    registry.register_type(false, STRUCTURE_FEATURE, |_| remainder());
    registry.register_type(false, OBJECTIVE, |_| remainder());
    registry.register_type(false, TEAM, |s| {
        optional_fields(vec![
            ("MemberNamePrefix", TEXT_COMPONENT.in_schema(s)),
            ("MemberNameSuffix", TEXT_COMPONENT.in_schema(s)),
            ("DisplayName", TEXT_COMPONENT.in_schema(s)),
        ])
    });
    registry.register_type(true, UNTAGGED_SPAWNER, |_| remainder());
    registry.register_type(false, POI_CHUNK, |_| remainder());
    registry.register_type(false, WORLD_GEN_SETTINGS, |_| remainder());
    registry.register_type(false, ENTITY_CHUNK, |s| optional_fields(vec![("Entities", list(ENTITY_TREE.in_schema(s)))]));
    registry.register_type(true, DATA_COMPONENTS, |_| remainder());
    registry.register_type(true, VILLAGER_TRADE, |s| {
        optional_fields(vec![("buy", ITEM_STACK.in_schema(s)), ("buyB", ITEM_STACK.in_schema(s)), ("sell", ITEM_STACK.in_schema(s))])
    });
    registry.register_type(true, PARTICLE, |_| string());
    registry.register_type(true, TEXT_COMPONENT, |_| string());
    registry.register_type(false, STRUCTURE, |s| {
        optional_fields(vec![
            ("entities", list(optional_fields(vec![("nbt", ENTITY_TREE.in_schema(s))]))),
            ("blocks", list(optional_fields(vec![("nbt", BLOCK_ENTITY.in_schema(s))]))),
            ("palette", list(BLOCK_STATE.in_schema(s))),
            ("palettes", list(list(BLOCK_STATE.in_schema(s)))),
        ])
    });
    registry.register_type(false, BLOCK_STATE, |_| remainder());
    registry.register_type(false, FLAT_BLOCK_STATE, |_| remainder());
    registry.register_type(true, ENTITY_EQUIPMENT, |s| optional(field("Equipment", list(ITEM_STACK.in_schema(s)))));
}

pub fn item_stack_tag(schema: &Schema) -> TypeTemplate {
    optional_fields(vec![
        ("EntityTag", ENTITY_TREE.in_schema(schema)),
        ("BlockEntityTag", BLOCK_ENTITY.in_schema(schema)),
        ("CanDestroy", list(BLOCK_NAME.in_schema(schema))),
        ("CanPlaceOn", list(BLOCK_NAME.in_schema(schema))),
        ("Items", list(ITEM_STACK.in_schema(schema))),
        ("ChargedProjectiles", list(ITEM_STACK.in_schema(schema))),
        ("pages", list(TEXT_COMPONENT.in_schema(schema))),
        ("filtered_pages", compound_list(TEXT_COMPONENT.in_schema(schema))),
        ("display", optional_fields(vec![("Name", TEXT_COMPONENT.in_schema(schema)), ("Lore", list(TEXT_COMPONENT.in_schema(schema)))])),
    ])
}

pub fn add_names(input: Dynamic, item_to_block_entity: fn(&str) -> Option<&'static str>, item_to_entity: fn(&str) -> Option<&'static str>) -> Dynamic {
    let item_id = input.get("id").as_string().into_result().map(|id| ensure_namespaced(&id)).unwrap_or_else(|_| "minecraft:air".to_string());
    let entity_item_id = ensure_namespaced(&input.get("id").as_string_or(""));
    input.update("tag", |item_stack_tag| {
        item_stack_tag
            .update("BlockEntityTag", |block_entity| {
                if item_id != "minecraft:air" {
                    if let Some(expected_id) = item_to_block_entity(&item_id) {
                        return block_entity.set("id", Dynamic::new(wafer_protocol::nbt::Nbt::String(expected_id.to_string())));
                    }
                    warn!("Unable to resolve BlockEntity for ItemStack: {item_id}");
                }
                block_entity
            })
            .update("EntityTag", |entity| {
                if entity.get("id").result().is_some() {
                    return entity;
                }
                match item_to_entity(&entity_item_id) {
                    Some(expected_id) => entity.set("id", Dynamic::new(wafer_protocol::nbt::Nbt::String(expected_id.to_string()))),
                    None => entity,
                }
            })
    })
}
