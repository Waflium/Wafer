use wafer_protocol::nbt::Nbt;

use crate::datafixers::dsl::TypeReference;
use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fixers::get_data_fixer;
use crate::util::datafix::fixes::references;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DataFixTypes {
    Level,
    LevelSummary,
    Player,
    Chunk,
    Hotbar,
    Options,
    Structure,
    Stats,
    SavedDataCommandStorage,
    SavedDataCustomBossEvents,
    SavedDataEnderDragonFight,
    SavedDataGameRules,
    SavedDataForcedChunks,
    SavedDataMapData,
    SavedDataMapIndex,
    SavedDataRaids,
    SavedDataRandomSequences,
    SavedDataScheduledEvents,
    SavedDataScoreboard,
    SavedDataStopwatches,
    SavedDataStructureFeatureIndices,
    SavedDataWanderingTrader,
    SavedDataWeather,
    SavedDataWorldBorder,
    SavedDataWorldClocks,
    SavedDataWorldGenSettings,
    Advancements,
    PoiChunk,
    WorldGenSettings,
    EntityChunk,
    DebugProfile,
}

impl DataFixTypes {
    pub fn type_reference(self) -> TypeReference {
        match self {
            DataFixTypes::Level => references::LEVEL,
            DataFixTypes::LevelSummary => references::LIGHTWEIGHT_LEVEL,
            DataFixTypes::Player => references::PLAYER,
            DataFixTypes::Chunk => references::CHUNK,
            DataFixTypes::Hotbar => references::HOTBAR,
            DataFixTypes::Options => references::OPTIONS,
            DataFixTypes::Structure => references::STRUCTURE,
            DataFixTypes::Stats => references::STATS,
            DataFixTypes::SavedDataCommandStorage => references::SAVED_DATA_COMMAND_STORAGE,
            DataFixTypes::SavedDataCustomBossEvents => references::SAVED_DATA_CUSTOM_BOSS_EVENTS,
            DataFixTypes::SavedDataEnderDragonFight => references::SAVED_DATA_ENDER_DRAGON_FIGHT,
            DataFixTypes::SavedDataGameRules => references::SAVED_DATA_GAME_RULES,
            DataFixTypes::SavedDataForcedChunks => references::SAVED_DATA_TICKETS,
            DataFixTypes::SavedDataMapData => references::SAVED_DATA_MAP_DATA,
            DataFixTypes::SavedDataMapIndex => references::SAVED_DATA_MAP_INDEX,
            DataFixTypes::SavedDataRaids => references::SAVED_DATA_RAIDS,
            DataFixTypes::SavedDataRandomSequences => references::SAVED_DATA_RANDOM_SEQUENCES,
            DataFixTypes::SavedDataScheduledEvents => references::SAVED_DATA_SCHEDULED_EVENTS,
            DataFixTypes::SavedDataScoreboard => references::SAVED_DATA_SCOREBOARD,
            DataFixTypes::SavedDataStopwatches => references::SAVED_DATA_STOPWATCHES,
            DataFixTypes::SavedDataStructureFeatureIndices => references::SAVED_DATA_STRUCTURE_FEATURE_INDICES,
            DataFixTypes::SavedDataWanderingTrader => references::SAVED_DATA_WANDERING_TRADER,
            DataFixTypes::SavedDataWeather => references::SAVED_DATA_WEATHER,
            DataFixTypes::SavedDataWorldBorder => references::SAVED_DATA_WORLD_BORDER,
            DataFixTypes::SavedDataWorldClocks => references::SAVED_DATA_WORLD_CLOCKS,
            DataFixTypes::SavedDataWorldGenSettings => references::SAVED_DATA_WORLD_GEN_SETTINGS,
            DataFixTypes::Advancements => references::ADVANCEMENTS,
            DataFixTypes::PoiChunk => references::POI_CHUNK,
            DataFixTypes::WorldGenSettings => references::WORLD_GEN_SETTINGS,
            DataFixTypes::EntityChunk => references::ENTITY_CHUNK,
            DataFixTypes::DebugProfile => references::DEBUG_PROFILE,
        }
    }

    pub fn update(self, input: Dynamic, from_version: i32, to_version: i32) -> Dynamic {
        get_data_fixer().update(self.type_reference(), input, from_version, to_version)
    }

    pub fn update_to_current_version(self, input: Dynamic, data_version: i32) -> Dynamic {
        self.update(input, data_version, current_version())
    }

    pub fn update_tag(self, tag: Nbt, from_version: i32, to_version: i32) -> Nbt {
        if from_version >= to_version {
            return tag;
        }
        self.update(Dynamic::new(tag), from_version, to_version).into_value().unwrap_or_else(|| Nbt::Compound(Vec::new()))
    }

    pub fn update_tag_to_current_version(self, tag: Nbt, from_version: i32) -> Nbt {
        self.update_tag(tag, from_version, current_version())
    }
}

pub fn current_version() -> i32 {
    wafer_data::WORLD_VERSION
}
