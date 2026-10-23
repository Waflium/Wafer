use std::sync::Arc;

use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::simplest_entity_rename_fix::SimplestEntityRenameFix;

pub const RENAMED_IDS: &[(&str, &str)] = &[
    ("minecraft:commandblock_minecart", "minecraft:command_block_minecart"),
    ("minecraft:ender_crystal", "minecraft:end_crystal"),
    ("minecraft:snowman", "minecraft:snow_golem"),
    ("minecraft:evocation_illager", "minecraft:evoker"),
    ("minecraft:evocation_fangs", "minecraft:evoker_fangs"),
    ("minecraft:illusion_illager", "minecraft:illusioner"),
    ("minecraft:vindication_illager", "minecraft:vindicator"),
    ("minecraft:villager_golem", "minecraft:iron_golem"),
    ("minecraft:xp_orb", "minecraft:experience_orb"),
    ("minecraft:xp_bottle", "minecraft:experience_bottle"),
    ("minecraft:eye_of_ender_signal", "minecraft:eye_of_ender"),
    ("minecraft:fireworks_rocket", "minecraft:firework_rocket"),
];

pub const RENAMED_BLOCKS: &[(&str, &str)] = &[
    ("minecraft:portal", "minecraft:nether_portal"),
    ("minecraft:oak_bark", "minecraft:oak_wood"),
    ("minecraft:spruce_bark", "minecraft:spruce_wood"),
    ("minecraft:birch_bark", "minecraft:birch_wood"),
    ("minecraft:jungle_bark", "minecraft:jungle_wood"),
    ("minecraft:acacia_bark", "minecraft:acacia_wood"),
    ("minecraft:dark_oak_bark", "minecraft:dark_oak_wood"),
    ("minecraft:stripped_oak_bark", "minecraft:stripped_oak_wood"),
    ("minecraft:stripped_spruce_bark", "minecraft:stripped_spruce_wood"),
    ("minecraft:stripped_birch_bark", "minecraft:stripped_birch_wood"),
    ("minecraft:stripped_jungle_bark", "minecraft:stripped_jungle_wood"),
    ("minecraft:stripped_acacia_bark", "minecraft:stripped_acacia_wood"),
    ("minecraft:stripped_dark_oak_bark", "minecraft:stripped_dark_oak_wood"),
    ("minecraft:mob_spawner", "minecraft:spawner"),
];

pub const RENAMED_ITEMS: &[(&str, &str)] = &[
    ("minecraft:portal", "minecraft:nether_portal"),
    ("minecraft:oak_bark", "minecraft:oak_wood"),
    ("minecraft:spruce_bark", "minecraft:spruce_wood"),
    ("minecraft:birch_bark", "minecraft:birch_wood"),
    ("minecraft:jungle_bark", "minecraft:jungle_wood"),
    ("minecraft:acacia_bark", "minecraft:acacia_wood"),
    ("minecraft:dark_oak_bark", "minecraft:dark_oak_wood"),
    ("minecraft:stripped_oak_bark", "minecraft:stripped_oak_wood"),
    ("minecraft:stripped_spruce_bark", "minecraft:stripped_spruce_wood"),
    ("minecraft:stripped_birch_bark", "minecraft:stripped_birch_wood"),
    ("minecraft:stripped_jungle_bark", "minecraft:stripped_jungle_wood"),
    ("minecraft:stripped_acacia_bark", "minecraft:stripped_acacia_wood"),
    ("minecraft:stripped_dark_oak_bark", "minecraft:stripped_dark_oak_wood"),
    ("minecraft:mob_spawner", "minecraft:spawner"),
    ("minecraft:clownfish", "minecraft:tropical_fish"),
    ("minecraft:chorus_fruit_popped", "minecraft:popped_chorus_fruit"),
    ("minecraft:evocation_illager_spawn_egg", "minecraft:evoker_spawn_egg"),
    ("minecraft:vindication_illager_spawn_egg", "minecraft:vindicator_spawn_egg"),
];

const MINECRAFT_BRED: &str = "minecraft:bred_";

pub struct EntityTheRenameningFix;

impl EntityTheRenameningFix {
    pub fn new(schema: &Arc<Schema>, changes_type: bool) -> SimplestEntityRenameFix {
        SimplestEntityRenameFix::new("EntityTheRenameningBlock", schema, changes_type, |name| {
            let name = match name.strip_prefix(MINECRAFT_BRED) {
                Some(rest) => format!("minecraft:{rest}"),
                None => name.to_string(),
            };
            RENAMED_IDS.iter().find(|(from, _)| *from == name).map_or(name.as_str(), |(_, to)| to).to_string()
        })
    }
}
