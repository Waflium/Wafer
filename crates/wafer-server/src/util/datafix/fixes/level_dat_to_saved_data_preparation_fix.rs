use std::sync::Arc;

use crate::datafixers::data_fix::{DataFix, DataFixBase, TypeRewriteRule};
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::fixes::references::LEVEL;

pub struct LevelDatToSavedDataPreparationFix {
    base: DataFixBase,
}

impl LevelDatToSavedDataPreparationFix {
    pub fn new(output_schema: &Arc<Schema>) -> LevelDatToSavedDataPreparationFix {
        LevelDatToSavedDataPreparationFix { base: DataFixBase::new(output_schema, true) }
    }
}

impl DataFix for LevelDatToSavedDataPreparationFix {
    fn base(&self) -> &DataFixBase {
        &self.base
    }

    fn make_rule(&self) -> TypeRewriteRule {
        let level_type = self.get_input_schema().get_type(LEVEL);
        self.write_fix_and_read("LevelSplitSavedDataPreparationFix", level_type, self.get_output_schema().get_type(LEVEL), |level_data| {
            let level_data = fix_dragon_fight(level_data);
            let level_data = fix_wandering_trader(level_data);
            let level_data = fix_weather_data(level_data);
            let level_data = fix_scheduled_events(level_data);
            fix_world_gen_settings(level_data)
        })
    }
}

fn fix_dragon_fight(level_data: Dynamic) -> Dynamic {
    let creator = level_data.clone();
    level_data.rename_and_fix_field("DragonFight", "dragon_fight", |dragon_fight| {
        let mut new_fight = dragon_fight
            .clone()
            .rename_field("NeedsStateScanning", "needs_state_scanning")
            .rename_field("DragonKilled", "dragon_killed")
            .rename_field("PreviouslyKilled", "previously_killed")
            .rename_field("Dragon", "dragon_uuid")
            .rename_field("ExitPortalLocation", "exit_portal_location")
            .rename_field("Gateways", "gateways");
        let is_respawning = dragon_fight.get("IsRespawning").as_boolean_or(false);
        if is_respawning {
            new_fight = new_fight.set("respawn_stage", creator.create_string("start")).set("respawn_time", creator.create_int(0));
        }
        new_fight.remove("IsRespawning")
    })
}

fn fix_wandering_trader(level_data: Dynamic) -> Dynamic {
    let wandering_trader_data = level_data
        .empty_map()
        .set("spawn_delay", level_data.create_int(level_data.get("WanderingTraderSpawnDelay").as_int(24000)))
        .set("spawn_chance", level_data.create_int(level_data.get("WanderingTraderSpawnChance").as_int(25)));
    level_data
        .set("wandering_trader_migration_data", wandering_trader_data)
        .remove("WanderingTraderSpawnDelay")
        .remove("WanderingTraderSpawnChance")
        .remove("WanderingTraderId")
}

fn fix_weather_data(level_data: Dynamic) -> Dynamic {
    let weather_data = level_data
        .empty_map()
        .set("clear_weather_time", level_data.create_int(level_data.get("clearWeatherTime").as_int(0)))
        .set("rain_time", level_data.create_int(level_data.get("rainTime").as_int(0)))
        .set("thunder_time", level_data.create_int(level_data.get("thunderTime").as_int(0)))
        .set("raining", level_data.create_boolean(level_data.get("raining").as_boolean_or(false)))
        .set("thundering", level_data.create_boolean(level_data.get("thundering").as_boolean_or(false)));
    level_data.remove("clearWeatherTime").remove("rainTime").remove("thunderTime").remove("raining").remove("thundering").set("weather_data", weather_data)
}

fn fix_scheduled_events(level_data: Dynamic) -> Dynamic {
    let scheduled_events = level_data.get("ScheduledEvents").as_list(|event| event);
    let event_list = level_data.create_list(scheduled_events.into_iter().map(|event| {
        event
            .rename_field("Name", "id")
            .rename_field("TriggerTime", "trigger_time")
            .rename_and_fix_field("Callback", "callback", |callback| callback.rename_field("Type", "type").rename_field("Name", "id"))
    }));
    let new_events = level_data.empty_map().set("events", event_list);
    level_data.remove("ScheduledEvents").set("scheduled_events", new_events)
}

fn fix_world_gen_settings(level_data: Dynamic) -> Dynamic {
    level_data.rename_and_fix_field("WorldGenSettings", "world_gen_settings", |tag| tag.rename_field("generate_features", "generate_structures"))
}
