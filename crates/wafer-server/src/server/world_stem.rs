use wafer_protocol::nbt::Nbt;

use super::bossevents::custom_boss_events::CustomBossEvents;
use super::reloadable_server_resources::ReloadableServerResources;
use super::players::PlayerLists;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::world::level::levelgen::world_dimensions::Complete;
use crate::world::level::levelgen::world_gen_settings::WorldGenSettings;
use crate::world::level::saveddata::weather_data::WeatherData;
use crate::world::level::storage::{WorldData, WorldDir, read_saved_data};
use crate::world::level::timers::timer_queue::TimerQueue;
use crate::world::random_sequences::RandomSequences;

pub struct WorldStem {
    pub data_packs: ReloadableServerResources,
    pub world_data: WorldData,
    pub gen_settings: WorldGenSettings,
    pub dimensions: Complete,
    pub saved_data: Option<ConstructorSavedData>,
    pub player_lists: Option<PlayerLists>,
}

pub struct ConstructorSavedData {
    pub random_sequences: RandomSequences,
    pub weather: Option<WeatherData>,
    pub game_rules: Option<Nbt>,
    pub clocks: Option<Nbt>,
    pub custom_boss_events: CustomBossEvents,
    pub scheduled_events: TimerQueue,
}

impl ConstructorSavedData {
    pub fn load(world_dir: Option<&WorldDir>) -> ConstructorSavedData {
        let random_sequences = RandomSequences::load(world_dir.map(WorldDir::data_folder));
        let file = |id: &str| world_dir.map(|dir| dir.data(id));
        let weather = file("weather")
            .and_then(|file| read_saved_data(&file, crate::server::main::SERVER_MAIN, "weather", DataFixTypes::SavedDataWeather, WeatherData::decode));
        let game_rules = file("game_rules")
            .and_then(|file| read_saved_data(&file, crate::server::main::SERVER_MAIN, "game_rules", DataFixTypes::SavedDataGameRules, crate::world::level::gamerules::GameRules::decode_saved));
        let clocks = file("world_clocks")
            .and_then(|file| read_saved_data(&file, crate::server::main::SERVER_MAIN, "world_clocks", DataFixTypes::SavedDataWorldClocks, crate::world::clock::ServerClockManager::decode_saved));
        let custom_boss_events = CustomBossEvents::load(world_dir.map(WorldDir::data_folder));
        let scheduled_events = TimerQueue::load(world_dir.map(WorldDir::data_folder));
        ConstructorSavedData { random_sequences, weather, game_rules, clocks, custom_boss_events, scheduled_events }
    }
}
