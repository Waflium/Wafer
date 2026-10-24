use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LEVEL;
use crate::util::java_integer::parse_int;

pub struct GameRuleRegistryFix {
    base: DataFixBase,
}

impl GameRuleRegistryFix {
    pub fn new(output_schema: &Arc<Schema>) -> GameRuleRegistryFix {
        GameRuleRegistryFix { base: DataFixBase::new(output_schema, false) }
    }
}

impl DataFix for GameRuleRegistryFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        self.fix_type_everywhere_typed("GameRuleRegistryFix", self.get_input_schema().get_type(LEVEL), |input| {
            input.update_remainder(|tag| tag.rename_and_fix_field("GameRules", "game_rules", fix_game_rules))
        })
    }
}

fn parse_boolean(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
}

fn fix_game_rules(game_rules: Dynamic) -> Dynamic {
    let do_fire_tick = parse_boolean(&game_rules.get("doFireTick").as_string_or("true"));
    let allow_fire_ticks_away_from_player = parse_boolean(&game_rules.get("allowFireTicksAwayFromPlayer").as_string_or("false"));
    let fire_spread_radius = if !do_fire_tick {
        0
    } else if !allow_fire_ticks_away_from_player {
        128
    } else {
        -1
    };
    let mut game_rules = game_rules;
    if fire_spread_radius != 128 {
        let radius = game_rules.create_int(fire_spread_radius);
        game_rules = game_rules.set("minecraft:fire_spread_radius_around_player", radius);
    }
    game_rules
        .remove("spawnChunkRadius")
        .remove("entitiesWithPassengersCanUsePortals")
        .remove("gameLoopFunction")
        .remove("doFireTick")
        .remove("allowFireTicksAwayFromPlayer")
        .rename_and_fix_field("allowEnteringNetherUsingPortals", "minecraft:allow_entering_nether_using_portals", convert_boolean)
        .rename_and_fix_field("announceAdvancements", "minecraft:show_advancement_messages", convert_boolean)
        .rename_and_fix_field("blockExplosionDropDecay", "minecraft:block_explosion_drop_decay", convert_boolean)
        .rename_and_fix_field("commandBlockOutput", "minecraft:command_block_output", convert_boolean)
        .rename_and_fix_field("enableCommandBlocks", "minecraft:command_blocks_work", convert_boolean)
        .rename_and_fix_field("commandBlocksEnabled", "minecraft:command_blocks_work", convert_boolean)
        .rename_and_fix_field("commandModificationBlockLimit", "minecraft:max_block_modifications", |old| convert_integer(old, 1, i32::MAX))
        .rename_and_fix_field("disableElytraMovementCheck", "minecraft:elytra_movement_check", convert_boolean_inverted)
        .rename_and_fix_field("disablePlayerMovementCheck", "minecraft:player_movement_check", convert_boolean_inverted)
        .rename_and_fix_field("disableRaids", "minecraft:raids", convert_boolean_inverted)
        .rename_and_fix_field("doDaylightCycle", "minecraft:advance_time", convert_boolean)
        .rename_and_fix_field("doEntityDrops", "minecraft:entity_drops", convert_boolean)
        .rename_and_fix_field("doImmediateRespawn", "minecraft:immediate_respawn", convert_boolean)
        .rename_and_fix_field("doInsomnia", "minecraft:spawn_phantoms", convert_boolean)
        .rename_and_fix_field("doLimitedCrafting", "minecraft:limited_crafting", convert_boolean)
        .rename_and_fix_field("doMobLoot", "minecraft:mob_drops", convert_boolean)
        .rename_and_fix_field("doMobSpawning", "minecraft:spawn_mobs", convert_boolean)
        .rename_and_fix_field("doPatrolSpawning", "minecraft:spawn_patrols", convert_boolean)
        .rename_and_fix_field("doTileDrops", "minecraft:block_drops", convert_boolean)
        .rename_and_fix_field("doTraderSpawning", "minecraft:spawn_wandering_traders", convert_boolean)
        .rename_and_fix_field("doVinesSpread", "minecraft:spread_vines", convert_boolean)
        .rename_and_fix_field("doWardenSpawning", "minecraft:spawn_wardens", convert_boolean)
        .rename_and_fix_field("doWeatherCycle", "minecraft:advance_weather", convert_boolean)
        .rename_and_fix_field("drowningDamage", "minecraft:drowning_damage", convert_boolean)
        .rename_and_fix_field("enderPearlsVanishOnDeath", "minecraft:ender_pearls_vanish_on_death", convert_boolean)
        .rename_and_fix_field("fallDamage", "minecraft:fall_damage", convert_boolean)
        .rename_and_fix_field("fireDamage", "minecraft:fire_damage", convert_boolean)
        .rename_and_fix_field("forgiveDeadPlayers", "minecraft:forgive_dead_players", convert_boolean)
        .rename_and_fix_field("freezeDamage", "minecraft:freeze_damage", convert_boolean)
        .rename_and_fix_field("globalSoundEvents", "minecraft:global_sound_events", convert_boolean)
        .rename_and_fix_field("keepInventory", "minecraft:keep_inventory", convert_boolean)
        .rename_and_fix_field("lavaSourceConversion", "minecraft:lava_source_conversion", convert_boolean)
        .rename_and_fix_field("locatorBar", "minecraft:locator_bar", convert_boolean)
        .rename_and_fix_field("logAdminCommands", "minecraft:log_admin_commands", convert_boolean)
        .rename_and_fix_field("maxCommandChainLength", "minecraft:max_command_sequence_length", |old| convert_integer(old, 0, i32::MAX))
        .rename_and_fix_field("maxCommandForkCount", "minecraft:max_command_forks", |old| convert_integer(old, 0, i32::MAX))
        .rename_and_fix_field("maxEntityCramming", "minecraft:max_entity_cramming", |old| convert_integer(old, 0, i32::MAX))
        .rename_and_fix_field("minecartMaxSpeed", "minecraft:max_minecart_speed", |old| convert_integer(old, i32::MIN, i32::MAX))
        .rename_and_fix_field("mobExplosionDropDecay", "minecraft:mob_explosion_drop_decay", convert_boolean)
        .rename_and_fix_field("mobGriefing", "minecraft:mob_griefing", convert_boolean)
        .rename_and_fix_field("naturalRegeneration", "minecraft:natural_health_regeneration", convert_boolean)
        .rename_and_fix_field("playersNetherPortalCreativeDelay", "minecraft:players_nether_portal_creative_delay", |old| convert_integer(old, 0, i32::MAX))
        .rename_and_fix_field("playersNetherPortalDefaultDelay", "minecraft:players_nether_portal_default_delay", |old| convert_integer(old, 0, i32::MAX))
        .rename_and_fix_field("playersSleepingPercentage", "minecraft:players_sleeping_percentage", |old| convert_integer(old, 0, i32::MAX))
        .rename_and_fix_field("projectilesCanBreakBlocks", "minecraft:projectiles_can_break_blocks", convert_boolean)
        .rename_and_fix_field("pvp", "minecraft:pvp", convert_boolean)
        .rename_and_fix_field("randomTickSpeed", "minecraft:random_tick_speed", |old| convert_integer(old, 0, i32::MAX))
        .rename_and_fix_field("reducedDebugInfo", "minecraft:reduced_debug_info", convert_boolean)
        .rename_and_fix_field("sendCommandFeedback", "minecraft:send_command_feedback", convert_boolean)
        .rename_and_fix_field("showDeathMessages", "minecraft:show_death_messages", convert_boolean)
        .rename_and_fix_field("snowAccumulationHeight", "minecraft:max_snow_accumulation_height", |old| convert_integer(old, 0, 8))
        .rename_and_fix_field("spawnMonsters", "minecraft:spawn_monsters", convert_boolean)
        .rename_and_fix_field("spawnRadius", "minecraft:respawn_radius", |old| convert_integer(old, i32::MIN, i32::MAX))
        .rename_and_fix_field("spawnerBlocksEnabled", "minecraft:spawner_blocks_work", convert_boolean)
        .rename_and_fix_field("spectatorsGenerateChunks", "minecraft:spectators_generate_chunks", convert_boolean)
        .rename_and_fix_field("tntExplodes", "minecraft:tnt_explodes", convert_boolean)
        .rename_and_fix_field("tntExplosionDropDecay", "minecraft:tnt_explosion_drop_decay", convert_boolean)
        .rename_and_fix_field("universalAnger", "minecraft:universal_anger", convert_boolean)
        .rename_and_fix_field("waterSourceConversion", "minecraft:water_source_conversion", convert_boolean)
}

fn convert_integer(old_value: Dynamic, min: i32, max: i32) -> Dynamic {
    let string_value = old_value.as_string_or("");
    match parse_int(&string_value) {
        Some(parsed_value) => old_value.create_int(parsed_value.clamp(min, max)),
        None => old_value,
    }
}

fn convert_boolean(old_value: Dynamic) -> Dynamic {
    old_value.create_boolean(parse_boolean(&old_value.as_string_or("")))
}

fn convert_boolean_inverted(old_value: Dynamic) -> Dynamic {
    old_value.create_boolean(!parse_boolean(&old_value.as_string_or("")))
}
