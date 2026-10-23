use crate::datafixers::dynamic::Dynamic;

use super::data_version::{DataVersion, MAIN_SERIES};

#[derive(Clone, Debug)]
pub struct LevelVersion {
    level_data_version: i32,
    last_played: i64,
    minecraft_version_name: String,
    minecraft_version: DataVersion,
    snapshot: bool,
}

impl LevelVersion {
    fn construct(level_data_version: i32, last_played: i64, minecraft_version_name: String, minecraft_version: i32, series: &str, snapshot: bool) -> LevelVersion {
        LevelVersion { level_data_version, last_played, minecraft_version_name, minecraft_version: DataVersion::new(minecraft_version, series), snapshot }
    }

    pub fn parse(input: &Dynamic) -> LevelVersion {
        let level_data_version = input.get("version").as_int(0);
        let last_played = input.get("LastPlayed").as_long(0);
        match input.get("Version").result() {
            Some(version) => LevelVersion::construct(
                level_data_version,
                last_played,
                version.get("Name").as_string_or(wafer_data::VERSION_NAME),
                version.get("Id").as_int(wafer_data::WORLD_VERSION),
                &version.get("Series").as_string_or(MAIN_SERIES),
                version.get("Snapshot").as_boolean_or(false),
            ),
            None => LevelVersion::construct(level_data_version, last_played, String::new(), 0, MAIN_SERIES, false),
        }
    }

    pub fn level_data_version(&self) -> i32 {
        self.level_data_version
    }

    pub fn last_played(&self) -> i64 {
        self.last_played
    }

    pub fn minecraft_version_name(&self) -> &str {
        &self.minecraft_version_name
    }

    pub fn minecraft_version(&self) -> &DataVersion {
        &self.minecraft_version
    }

    pub fn snapshot(&self) -> bool {
        self.snapshot
    }
}
