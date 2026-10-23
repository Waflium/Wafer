use std::sync::Arc;

use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::Schema;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::datafix::fixes::references;
use crate::util::filefix::access::compressed_nbt::CompressedNbtFile;
use crate::util::filefix::access::file_access::FileAccess;
use crate::util::filefix::access::file_relation::FileRelation;
use crate::util::filefix::access::file_resource_types::FileResourceTypes;
use crate::util::filefix::file_fix::FileFix;
use crate::util::worldupdate::upgrade_progress::Type;

const FALLBACK_SINGLE_PLAYER_UUID: &str = "00000000-0000-0000-0000-000000000000";
const OVERWORLD: &str = "overworld";
const THE_NETHER: &str = "the_nether";
const THE_END: &str = "the_end";
const WORLD_BORDER_KEY: &str = "world_border";
const WORLD_BORDER_FILE_NAME: &str = "minecraft/world_border.dat";

pub struct LevelDatToSavedDataFileFix;

impl LevelDatToSavedDataFileFix {
    pub fn new(schema: &Arc<Schema>) -> FileFix {
        let mut file_fix = FileFix::new(schema);
        let version = file_fix.get_version();
        file_fix.add_file_content_fix(move |files| {
            let level_dat = files.get_file_access(FileResourceTypes::level_dat(), FileRelation::origin().for_file("level.dat"));
            let dragon_fight = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_ENDER_DRAGON_FIGHT),
                FileRelation::for_data_file_in_dimension(THE_END, "minecraft/ender_dragon_fight.dat"),
            );
            let fallback_player_data = files.get_file_access(
                FileResourceTypes::player_data(),
                FileRelation::player_data().for_file(&format!("{FALLBACK_SINGLE_PLAYER_UUID}.dat")),
            );
            let wandering_trader = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_WANDERING_TRADER),
                FileRelation::data().for_file("minecraft/wandering_trader.dat"),
            );
            let custom_boss_events = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_CUSTOM_BOSS_EVENTS),
                FileRelation::data().for_file("minecraft/custom_boss_events.dat"),
            );
            let weather_data = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_WEATHER),
                FileRelation::data().for_file("minecraft/weather.dat"),
            );
            let scheduled_events = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_SCHEDULED_EVENTS),
                FileRelation::data().for_file("minecraft/scheduled_events.dat"),
            );
            let world_border_overworld = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_WORLD_BORDER),
                FileRelation::for_data_file_in_dimension(OVERWORLD, WORLD_BORDER_FILE_NAME),
            );
            let world_border_nether = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_WORLD_BORDER),
                FileRelation::for_data_file_in_dimension(THE_NETHER, WORLD_BORDER_FILE_NAME),
            );
            let world_border_end = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_WORLD_BORDER),
                FileRelation::for_data_file_in_dimension(THE_END, WORLD_BORDER_FILE_NAME),
            );
            let game_rules = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_GAME_RULES),
                FileRelation::data().for_file("minecraft/game_rules.dat"),
            );
            let world_gen_settings = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_WORLD_GEN_SETTINGS),
                FileRelation::data().for_file("minecraft/world_gen_settings.dat"),
            );
            let world_clocks = files.get_file_access(
                FileResourceTypes::saved_data(references::SAVED_DATA_WORLD_CLOCKS),
                FileRelation::data().for_file("minecraft/world_clocks.dat"),
            );
            Box::new(move |upgrade_progress| {
                upgrade_progress.set_type(Type::Files);
                let level_dat_file = level_dat.get_only_file();
                let read_data = level_dat_file.read()?;
                if let Some(mut content) = read_data {
                    content = extract_to_file(&dragon_fight, content, "dragon_fight");
                    content = extract_player_data_to_file(version, &fallback_player_data, content);
                    content = extract_to_file(&wandering_trader, content, "wandering_trader_migration_data");
                    content = extract_to_file(&custom_boss_events, content, "CustomBossEvents");
                    content = extract_to_file(&weather_data, content, "weather_data");
                    content = extract_to_file(&scheduled_events, content, "scheduled_events");
                    content = extract_world_border_to_files(&world_border_overworld, &world_border_nether, &world_border_end, content);
                    content = extract_to_file(&game_rules, content, "game_rules");
                    content = extract_world_gen_settings_to_file(version, &world_gen_settings, content);
                    content = extract_to_file(&world_clocks, content, "world_clocks");
                    level_dat_file.write(&content);
                }
                Ok(())
            })
        });
        file_fix
    }
}

fn extract_to_file<T: CompressedNbtFile>(target_file: &FileAccess<T>, content: Dynamic, key: &str) -> Dynamic {
    let Some(tag) = content.get(key).result() else {
        return content;
    };
    target_file.get_only_file().write(&tag);
    content.remove(key)
}

fn extract_player_data_to_file<T: CompressedNbtFile>(version: i32, fallback_file: &FileAccess<T>, content: Dynamic) -> Dynamic {
    let Some(player_tag) = content.get("Player").result() else {
        return content;
    };
    let data_version = player_tag.get("DataVersion").as_int(-1);
    let player_tag_fixed = DataFixTypes::Player.update(player_tag, data_version, version);
    let used_uuid = match player_tag_fixed.get("UUID").result() {
        Some(player_uuid) => player_uuid,
        None => {
            fallback_file.get_only_file().write(&player_tag_fixed);
            content.create_int_list([0, 0, 0, 0])
        }
    };
    content.remove("Player").set("singleplayer_uuid", used_uuid)
}

fn extract_world_border_to_files<T: CompressedNbtFile>(
    world_border_overworld: &FileAccess<T>,
    world_border_nether: &FileAccess<T>,
    world_border_end: &FileAccess<T>,
    content: Dynamic,
) -> Dynamic {
    extract_world_border_to_file(world_border_overworld, &content, 1.0);
    extract_world_border_to_file(world_border_nether, &content, 8.0);
    extract_world_border_to_file(world_border_end, &content, 1.0);
    content.remove(WORLD_BORDER_KEY)
}

fn extract_world_border_to_file<T: CompressedNbtFile>(target_file: &FileAccess<T>, content: &Dynamic, divider: f64) {
    if let Some(world_border_tag) = content.get(WORLD_BORDER_KEY).result() {
        let world_border_tag = world_border_tag
            .update("center_x", |x| x.create_double(x.as_double(0.0) / divider))
            .update("center_z", |z| z.create_double(z.as_double(0.0) / divider));
        target_file.get_only_file().write(&world_border_tag);
    }
}

fn extract_world_gen_settings_to_file<T: CompressedNbtFile>(version: i32, target_file: &FileAccess<T>, content: Dynamic) -> Dynamic {
    let Some(world_gen_settings_tag) = content.get("world_gen_settings").result() else {
        return content;
    };
    let data_version = content.get("DataVersion").as_int(-1);
    let world_gen_settings_tag_fixed = DataFixTypes::WorldGenSettings.update(world_gen_settings_tag, data_version, version);
    target_file.get_only_file().write(&world_gen_settings_tag_fixed);
    content.remove("world_gen_settings")
}
