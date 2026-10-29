pub mod advancements;
pub mod bossevents;
pub mod command_frames;
pub mod command_origin;
pub mod commands;
pub mod dedicated;
pub mod dialog;
pub mod level;
pub mod network;
pub mod packs;
pub mod players;
pub mod rcon;
pub mod reloadable_server_registries;
pub mod reloadable_server_resources;
pub mod eula;
pub mod jsonrpc;
pub mod main;
pub mod notifications;
pub mod player_advancements;
pub mod server_advancement_manager;
pub mod server_tick_rate_manager;
pub mod services;
pub mod suppressed_exception_collector;
#[cfg(test)]
mod scoreboard_tests;
pub mod server_function_library;
pub mod server_function_manager;
pub mod server_links;
pub mod server_scoreboard;
pub mod waypoints;
pub mod world_stem;

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use wafer_core::mth;
use wafer_core::random::{LegacyRandomSource, RandomSource};

use wafer_core::BlockPos;
use wafer_data::packets::play::{clientbound, serverbound};
use wafer_data::{SyncedRegistry, game_rule};
use wafer_protocol::nbt::{Nbt, text};
use sha2::{Digest, Sha256};
use tokio::sync::mpsc;

use crate::network::chat::last_seen_messages::LastSeenMessages;
use crate::network::chat::message_signature::MessageSignature;
use crate::network::chat::signed_message_body::SignedMessageBody;
use crate::server::network::chat::{ArgumentSignature, ChatState};
use crate::{LevelType, ServerConfig};
use crate::commands::arguments::selector::entity_world::EntityWorld;
use crate::commands::source::CommandSourceStack;
use crate::network::chat::{ChatFormatting, Component, player_joined_or_left};
use crate::network::configuration::{ChatVisibility, CommonListenerCookie};
use crate::network::server_status::Favicon;
use crate::network::connection::{Outbound, Packet};
use crate::network::login::{Profile, Verdict};
use crate::util::java_throwable::JavaThrowable;
use crate::world::clock::ServerClockManager;
use crate::world::entity::EntityId;
use crate::util::lifecycle::Lifecycle;
use crate::world::level::dimension::level_stem::{DimensionTypeHolder, LevelStem};
use crate::world::level::levelgen::presets::world_presets::{self, FLAT, NORMAL};
use crate::world::level::levelgen::world_dimensions::SpecialWorldProperty;
use crate::world::level::levelgen::world_gen_settings::WorldGenSettings;
use crate::world::level::levelgen::world_options::WorldOptions;
use crate::world::level::storage::level_storage_source::{LevelDataAndDimensions, get_level_data_and_dimensions};
use command_origin::CommandOrigin;
use crate::world::level::chunk::storage::RegionStorage;
use crate::world::level::chunk::storage::chunk_io_error_reporter::ChunkIoErrorReporter;
use crate::world::level::chunk::storage::region_storage_info::RegionStorageInfo;
use level::chunk_workers::IoReporting;
use crate::world::level::gamerules::GameRules;
use crate::world::level::border::border_change_listener::write_initialize_border;
use crate::world::level::border::world_border::WorldBorder;
use crate::world::difficulty::Difficulty;
use crate::world::level::storage::format_uuid;
use crate::server::services::Services;
use crate::world::level::storage::command_storage::CommandStorage;
use crate::world::level::storage::saved_data_storage::SavedDataStorage;
use crate::world::random_sequences::RandomSequences;
use crate::world::level::timers::timer_queue::TimerQueue;
use self::players::{NameAndId, PlayerLists};
use self::players::cached_user_name_to_id_resolver::CachedUserNameToIdResolver;
use crate::world::stopwatches::Stopwatches;
use self::server_tick_rate_manager::ServerTickRateManager;
use self::bossevents::custom_boss_events::CustomBossEvents;
use crate::world::level::saveddata::weather_data::WeatherDataCell;
use crate::world::scores::scoreboard::Scoreboard;
use crate::world::scores::scoreboard_save_data::ScoreboardSaveData;
use crate::world::level::storage::{DirtySavedData, RespawnData, WorldData, WorldDir};
use level::ServerLevel;
use self::network::prepare_spawn_task::PrepareSpawnTask;
use level::game_mode::GameType;
use level::chunk_cache::ChunkCache;
use level::server_player::ServerPlayer;
use self::packs::PackSource;
use self::packs::repository::PackRepository;
use self::packs::resources::MultiPackResourceManager;
use self::reloadable_server_resources::ReloadableServerResources;
use self::world_stem::{ConstructorSavedData, WorldStem};
use self::server_function_manager::ServerFunctionManager;
use self::jsonrpc::management_server::ManagementServer;
use self::notifications::NotificationManager;
use self::notifications::server_activity_monitor::ServerActivityMonitor;
use crate::network::server_connection_listener::ListenerSettings;
use crate::world::flag::{FeatureFlagSet, print_missing_flags};
use crate::world::level::data_pack_config::DataPackConfig;
use crate::world::level::world_data_configuration::WorldDataConfiguration;

pub const OVERWORLD: &str = "minecraft:overworld";
pub const THE_NETHER: &str = "minecraft:the_nether";
pub const THE_END: &str = "minecraft:the_end";

fn system_report_snapshot(pack_repository: &PackRepository, enabled_features: FeatureFlagSet, seed: i64, reporter: Option<ChunkIoErrorReporter>) -> dedicated::server_watchdog::ServerSnapshot {
    dedicated::server_watchdog::ServerSnapshot {
        world_seed: seed,
        active_data_packs: PackRepository::display_pack_list(pack_repository.selected_packs()),
        available_data_packs: PackRepository::display_pack_list(pack_repository.available_packs()),
        enabled_feature_flags: enabled_features.to_names().join(", "),
        chunk_io_error_reporter: reporter,
        ..Default::default()
    }
}

#[allow(clippy::too_many_arguments)]
fn create_level(
    config: &ServerConfig,
    world_dir: Option<&WorldDir>,
    dimension: &'static str,
    level_stem: &LevelStem,
    is_debug: bool,
    generate_structures: bool,
    seed: i64,
    chunk_io_error_reporter: &ChunkIoErrorReporter,
) -> ServerLevel {
    let generator = level_stem.generator.create(seed);
    let (dimension_type_id, dimension_type) = match &level_stem.dimension_type {
        DimensionTypeHolder::Reference(dimension_type) => {
            let dimension_type_id = SyncedRegistry::get("minecraft:dimension_type").id_of(dimension_type).expect("a dimension type");
            (dimension_type_id, &wafer_data::dimension_types()[dimension_type_id as usize])
        }
        DimensionTypeHolder::Direct(tag) => (-1, crate::core::data_registries::inline_dimension_type(dimension, tag)),
    };
    let storage = world_dir.map(|d| RegionStorage::new(d.regions(dimension), config.sync_chunk_writes));
    let generator_identifier = generator.get_type_name_for_data_fixer();
    let mut chunks = ChunkCache::new(dimension_type, generator, storage);
    chunks.data_fix_context = Some(level::chunk_map::get_chunk_data_fix_context_tag(dimension, generator_identifier));
    chunks.is_debug = is_debug;
    chunks.generate_structures = generate_structures;
    chunks.set_seed(seed);
    if let Some(dir) = world_dir {
        chunks.set_poi_storage(RegionStorage::new(dir.poi(dimension), config.sync_chunk_writes));
    }
    let entity_storage = world_dir.map(|d| RegionStorage::new(d.entities(dimension), config.sync_chunk_writes));
    let mut level = ServerLevel::new(dimension, dimension_type_id, chunks, entity_storage, config.view_distance, config.simulation_distance);
    if let Some(dir) = world_dir {
        level.chunks.set_error_reporting(chunk_io_error_reporter.clone(), &config.level_name, dimension);
        level.entity_manager.set_reporting(IoReporting { reporter: chunk_io_error_reporter.clone(), info: RegionStorageInfo::new(&config.level_name, dimension, "entities") });
        level.load_chunk_tickets(dir);
    }
    level.biome_zoom_seed = crate::world::level::biome::obfuscate_seed(seed);
    level.world_border = WorldBorder::load(world_dir.map(|d| d.dimension_data_folder(dimension)));
    level.world_border.set_absolute_max_size(config.max_world_size);
    level.raids = crate::world::entity::raid::raids::Raids::load(world_dir.map(|d| d.dimension_data_folder(dimension)));
    if dimension_type.has_ender_dragon_fight {
        level.dragon_fight = Some(crate::world::level::dimension::end::ender_dragon_fight::EnderDragonFight::load(world_dir.map(|d| d.dimension_data_folder(dimension))));
        level.dragon_fight_init(seed, wafer_core::BlockPos::ZERO);
    }
    level.allow_flight = config.allow_flight;
    level.set_max_chained_neighbor_updates(config.max_chained_neighbor_updates);
    level.entity_broadcast_range_percentage = config.entity_broadcast_range_percentage;
    level.seed = seed;
    if dimension == OVERWORLD {
        level.custom_spawners = vec![
            crate::world::level::custom_spawner::CustomSpawner::Phantom(crate::world::level::levelgen::phantom_spawner::PhantomSpawner::default()),
            crate::world::level::custom_spawner::CustomSpawner::Patrol(crate::world::level::levelgen::patrol_spawner::PatrolSpawner::default()),
            crate::world::level::custom_spawner::CustomSpawner::Cat(crate::world::entity::npc::cat_spawner::CatSpawner::default()),
            crate::world::level::custom_spawner::CustomSpawner::VillageSiege(crate::world::entity::ai::village::village_siege::VillageSiege::default()),
            crate::world::level::custom_spawner::CustomSpawner::WanderingTrader(crate::world::entity::npc::wanderingtrader::wandering_trader_spawner::WanderingTraderSpawner::default()),
        ];
    }
    level
}
const AUTOSAVE_INTERVAL: i32 = 6000;
const STATUS_EXPIRE_TIME: Duration = Duration::from_secs(5);
const MAX_STATUS_PLAYER_SAMPLE: usize = 12;
const EVENT_PERMISSION_LEVEL_ALL: u8 = 24;
const GAME_EVENT_LIMITED_CRAFTING: u8 = 12;
const GAME_EVENT_LEVEL_CHUNKS_LOAD_START: u8 = 13;
const GAME_EVENT_IMMEDIATE_RESPAWN: u8 = 11;

pub type ConnId = u64;

pub enum Event {
    Join { conn: ConnId, profile: Profile, cookie: CommonListenerCookie, out: Outbound, address: std::net::SocketAddr },
    Packet { conn: ConnId, packet: Packet },
    Left { conn: ConnId, reason: Component },
    Console(String, bool),
    Verify { profile: Profile, address: std::net::SocketAddr, user_name: String, reply: tokio::sync::oneshot::Sender<Verdict> },
    PrepareSpawn { uuid: u128, name: String, reply: tokio::sync::oneshot::Sender<Result<(), ()>> },
    PrepareSpawnClosed { uuid: u128 },
    Halt,
    Rcon { command: String, reply: tokio::sync::oneshot::Sender<String> },
}

pub type ServerTask = Box<dyn FnOnce(&mut Server) + Send + Sync>;

pub(crate) enum Task {
    Chat { player: EntityId, body: SignedMessageBody, signature: Option<MessageSignature> },
    Command { player: EntityId, command: String },
    SignedCommand { player: EntityId, command: String, time_stamp: i64, salt: i64, arguments: Vec<ArgumentSignature>, last_seen: LastSeenMessages },
    Chained { player: EntityId, run: Box<dyn FnOnce(&mut Server)> },
    CommandSuggestions { player: EntityId },
    Rcon { command: String, reply: tokio::sync::oneshot::Sender<String> },
}

struct PendingLogin {
    profile: Profile,
    address: std::net::SocketAddr,
    user_name: String,
    reply: tokio::sync::oneshot::Sender<Verdict>,
    waiting_for_dupe: bool,
}

pub struct Server {
    pub config: ServerConfig,
    pub settings: Option<dedicated::dedicated_server_settings::DedicatedServerSettings>,
    pub levels: Vec<ServerLevel>,
    pub player_list: Vec<EntityId>,
    next_player_list_order: u64,
    player_names_array: Arc<RwLock<Arc<[String]>>>,
    connections: HashMap<ConnId, EntityId>,
    lost_connections: HashMap<ConnId, Component>,
    pub clocks: ServerClockManager,
    pub game_rules: GameRules,
    pub weather: Rc<WeatherDataCell>,
    pub scoreboard: Rc<RefCell<Scoreboard>>,
    pub command_storage: Rc<RefCell<CommandStorage>>,
    pub random_sequences: Rc<RefCell<RandomSequences>>,
    pub data_storage: Rc<RefCell<SavedDataStorage>>,
    pub ops: players::ServerOpList,
    pub bans: players::StoredUserList<players::UserBanEntry>,
    pub ip_bans: players::StoredUserList<players::IpBanEntry>,
    pub whitelist: players::StoredUserList<players::WhiteListEntry>,
    empty_ticks: i32,
    send_all_player_info_in: i32,
    pub seed: i64,
    pub tick_count: i32,
    debug_command_profiler: Option<commands::debug_command::TimeProfiler>,
    pub(crate) game_test_ticker: crate::gametest::framework::game_test_ticker::GameTestTicker,
    debug_command_profiler_delay_start: bool,
    ticks_until_autosave: i32,
    world_gen_settings_dirty: bool,
    pub tick_rate_manager: ServerTickRateManager,
    tick_times_nanos: [i64; 100],
    aggregated_tick_times_nanos: i64,
    pending_logins: Vec<PendingLogin>,
    login_disconnections_to_report: usize,
    prepare_spawn_tasks: Vec<PrepareSpawnTask>,
    tasks: VecDeque<Task>,
    console_inputs: Vec<(String, CommandSourceStack, bool)>,
    pub rcon_buffer: std::cell::RefCell<String>,
    pub command_origin: command_origin::CommandOrigin,
    pub command_block_messages: std::cell::RefCell<Vec<(u64, crate::network::chat::Component)>>,
    random: LegacyRandomSource,
    status: Arc<RwLock<Option<Arc<str>>>>,
    last_status: Option<Instant>,
    status_icon: Option<Favicon>,
    session_id: Option<u128>,
    #[cfg(test)]
    pub console_log: std::cell::RefCell<Vec<String>>,
    pub spawn: RespawnData,
    effective_respawn_data: RespawnData,
    pub default_game_type: Rc<Cell<GameType>>,
    pub difficulty: Difficulty,
    pub difficulty_locked: bool,
    pub hardcore: bool,
    generate_structures: bool,
    world_gen_settings: WorldGenSettings,
    world_gen_settings_lifecycle: Lifecycle,
    special_world_property: SpecialWorldProperty,
    bonus_chest: bool,
    server_brands: Vec<String>,
    was_modded: bool,
    level_name: String,
    allow_commands: bool,
    singleplayer_uuid: Option<Nbt>,
    pub scheduled_events: TimerQueue,
    pub name_to_id_cache: Arc<CachedUserNameToIdResolver>,
    pub profile_resolver: Arc<players::profile_resolver::ProfileResolver>,
    pub task_sender: Option<std::sync::mpsc::Sender<ServerTask>>,
    pub custom_boss_events: CustomBossEvents,
    pub stopwatches: Stopwatches,
    pub services: Services,
    pub functions: ServerFunctionManager,
    pub pack_repository: PackRepository,
    pub data_configuration: WorldDataConfiguration,
    pub scoreboard_save_data: ScoreboardSaveData,
    pub notification_manager: NotificationManager,
    pub server_activity_monitor: Arc<ServerActivityMonitor>,
    pub listener_settings: Arc<ListenerSettings>,
    world_dir: Option<WorldDir>,
    pub chunk_io_error_reporter: ChunkIoErrorReporter,
    stopping: bool,
    ready: bool,
}

impl Server {
    pub(crate) fn new(config: ServerConfig) -> std::io::Result<Server> {
        Server::new_with_player_lists(config, None)
    }

    pub(crate) fn new_with_player_lists(config: ServerConfig, player_lists: Option<PlayerLists>) -> std::io::Result<Server> {
        let saved = match &config.world {
            Some(root) => WorldData::load_level_tag(&WorldDir { root: root.clone() })?,
            None => None,
        };
        let mut pack_repository = match &config.world {
            Some(root) => {
                let validator = crate::world::level::storage::parse_validator(&root.parent().unwrap_or(Path::new("")).join(crate::world::level::storage::ALLOWED_SYMLINKS_CONFIG_NAME));
                PackRepository::create_pack_repository(root.join("datapacks"), Arc::new(validator))
            }
            None => PackRepository::create_vanilla_trusted_repository(),
        };
        let initial_data_config = saved.as_ref().map_or_else(WorldDataConfiguration::default_configuration, WorldDataConfiguration::parse);
        let data_configuration = configure_pack_repository(&mut pack_repository, &initial_data_config, saved.is_none(), false);
        let resource_manager = MultiPackResourceManager::new(pack_repository.open_all_selected());
        crate::core::data_registries::load(&resource_manager);
        let postponed_tags = wafer_data::prepare_tags(crate::tags::load_tags_for_existing_registries(&resource_manager, crate::tags::ExistingTagLoading::Startup));
        wafer_data::with_pending_tags(postponed_tags, || crate::core::data_registries::validate(&resource_manager)).map_err(std::io::Error::other)?;
        let mut data_packs = ReloadableServerResources::load_resources(&resource_manager, postponed_tags, config.function_permission_level).map_err(std::io::Error::other)?;
        data_packs.update_components_and_static_registry_tags();
        data_packs.finalize_recipe_loading();
        crate::world::level::levelgen::structure::templatesystem::structure_template_manager::on_resource_manager_reload(&resource_manager);
        let datapack_dimensions = crate::core::data_registries::datapack_dimensions();
        let registries_lifecycle = crate::core::data_registries::registries_lifecycle();
        let level_data_and_dimensions = match (saved, &config.world) {
            (Some(level_data_tag), Some(root)) => {
                get_level_data_and_dimensions(&WorldDir { root: root.clone() }, &level_data_tag, data_configuration, &datapack_dimensions, registries_lifecycle, &[]).map_err(std::io::Error::other)?
            }
            _ => {
                let key = if config.level_type == LevelType::Flat { FLAT } else { NORMAL };
                let dimensions = world_presets::get(key).expect("a world preset").create_world_dimensions();
                let options = WorldOptions::new(config.seed.unwrap_or_else(WorldOptions::random_seed), true, false);
                let final_dimensions = dimensions.bake(&datapack_dimensions);
                let lifecycle = final_dimensions.lifecycle().add(registries_lifecycle);
                let world_data = WorldData::new_world(config.game_mode, config.difficulty, false, data_configuration, config.level_name.clone(), final_dimensions.special_world_property, lifecycle);
                LevelDataAndDimensions { world_data, gen_settings: WorldGenSettings::new(options, dimensions), dimensions: final_dimensions }
            }
        };
        let LevelDataAndDimensions { world_data, gen_settings, dimensions } = level_data_and_dimensions;
        Server::with_world_stem(config, pack_repository, WorldStem { data_packs, world_data, gen_settings, dimensions, saved_data: None, player_lists })
    }

    pub(crate) fn with_world_stem(config: ServerConfig, pack_repository: PackRepository, world_stem: WorldStem) -> std::io::Result<Server> {
        let WorldStem { data_packs, world_data: data, gen_settings, dimensions, saved_data, player_lists } = world_stem;
        let data_configuration = data.data_configuration.clone();
        let new_world = !data.initialized;
        let world_dir = config.world.clone().map(|root| WorldDir { root });
        let chunk_io_error_reporter = ChunkIoErrorReporter::new(PathBuf::new(), world_dir.as_ref().map(|dir| dir.root.clone()));
        crate::world::level::levelgen::structure::templatesystem::structure_template_manager::set_generated_dir(world_dir.as_ref().map(WorldDir::generated));
        let saved_data = saved_data.unwrap_or_else(|| ConstructorSavedData::load(world_dir.as_ref()));
        let ConstructorSavedData { random_sequences, weather, game_rules: saved_game_rules, clocks: saved_clocks, custom_boss_events, scheduled_events } = saved_data;
        let mut clocks = ServerClockManager::new();
        let game_rules = GameRules::new(data_configuration.enabled_features);
        crate::commands::set_enabled_features(data_configuration.enabled_features);
        let game_rules_changed = game_rules.load(&saved_game_rules.unwrap_or_else(|| Nbt::Compound(Vec::new())));
        game_rules.set_dirty(game_rules_changed);
        if let Some(saved_clocks) = &saved_clocks {
            clocks.load(saved_clocks);
        }
        clocks.dirty = saved_clocks.is_none();
        let weather_dirty = weather.is_none();
        let weather = weather.unwrap_or_default();
        let generate_structures = gen_settings.options().generate_structures();
        let bonus_chest = gen_settings.options().generate_bonus_chest();
        let (mut server_brands, was_modded) = (data.server_brands, data.was_modded);
        let (level_name, allow_commands, singleplayer_uuid) = (data.level_name, data.allow_commands, data.singleplayer_uuid);
        if !server_brands.iter().any(|b| b == "vanilla") {
            server_brands.push("vanilla".into());
        }
        let mut load_listener = level::progress::LoggingLevelLoadListener::for_dedicated_server();
        let spawn = (!new_world).then_some(data.spawn);
        let special_world_property = data.special_world_property;
        let is_debug = special_world_property == SpecialWorldProperty::Debug;
        let seed = gen_settings.options().seed();
        let (game_time, mut game_type, mut difficulty, mut difficulty_locked, hardcore) = (data.game_time, config.game_mode, data.difficulty, data.difficulty_locked, data.hardcore);
        let world_gen_settings_lifecycle = data.world_gen_settings_lifecycle;
        let mut stems = dimensions.levels_in_creation_order().into_iter();
        let (_, overworld_stem) = stems.next().expect("overworld settings");
        let mut overworld = create_level(&config, world_dir.as_ref(), OVERWORLD, &overworld_stem, is_debug, generate_structures, seed, &chunk_io_error_reporter);
        overworld.chunks.game_rules = game_rules.clone();
        dedicated::set_overworld_created();
        let mut scoreboard_save_data = ScoreboardSaveData::load(world_dir.as_ref().map(WorldDir::data_folder));
        let scoreboard = Rc::new(RefCell::new(Scoreboard::default()));
        if let Err(throwable) = scoreboard.borrow_mut().load(scoreboard_save_data.take_packed()) {
            let saved_levels = vec![format!("'ServerLevel[{level_name}]'/minecraft:overworld")];
            return Err(std::io::Error::other(StartupFailure { throwable, saving_players: true, saved_levels, snapshot: Some(dedicated::server_watchdog::ServerSnapshot { player_count: Some(format!("0 / {}; []", config.max_players)), world_gen_settings_lifecycle, ..system_report_snapshot(&pack_repository, data_configuration.enabled_features, seed, None) }) }));
        }
        let command_storage = Rc::new(RefCell::new(CommandStorage::new(world_dir.as_ref().map(WorldDir::data_folder))));
        let stopwatches = Stopwatches::load(world_dir.as_ref().map(WorldDir::data_folder));
        if new_world && !is_debug {
            load_listener.start(level::progress::Stage::PrepareGlobalSpawn, 0);
        }
        let spawn = match spawn {
            Some(spawn) => spawn,
            None if is_debug => RespawnData::of(OVERWORLD, BlockPos::new(0, 80, 0), 0.0, 0.0),
            None => {
                let spawn = RespawnData::of(OVERWORLD, level::player_spawn_finder::initial_spawn(&mut overworld), 0.0, 0.0);
                if bonus_chest && let Some(feature) = crate::world::level::levelgen::feature::feature::feature("minecraft:bonus_chest") {
                    let mut random = std::mem::replace(&mut overworld.random, wafer_core::random::LegacyRandomSource::new(0));
                    feature.place(&mut overworld, &mut random, spawn.pos);
                    overworld.random = random;
                }
                spawn
            }
        };
        if new_world && !is_debug {
            load_listener.finish(level::progress::Stage::PrepareGlobalSpawn);
        }
        if new_world && is_debug {
            difficulty = crate::world::difficulty::Difficulty::Peaceful;
            difficulty_locked = true;
            game_rules.set(wafer_data::game_rule::ADVANCE_WEATHER, 0);
            clocks.move_to_time_marker(ServerClockManager::id_of(OVERWORLD).unwrap(), "minecraft:noon");
            game_type = GameType::Spectator;
        }
        let mut levels = vec![overworld];
        for (key, stem) in stems {
            let dimension: &'static str = match key.as_str() {
                THE_END => THE_END,
                THE_NETHER => THE_NETHER,
                _ => Box::leak(key.into_boxed_str()),
            };
            levels.push(create_level(&config, world_dir.as_ref(), dimension, &stem, is_debug, generate_structures, seed, &chunk_io_error_reporter));
        }
        for (index, level) in levels.iter_mut().enumerate() {
            level.index = index;
            level.level_name = level_name.clone();
        }
        let weather = Rc::new(WeatherDataCell::new(weather, weather_dirty));
        let default_game_type = Rc::new(Cell::new(game_type));
        let random_sequences = Rc::new(RefCell::new(random_sequences));
        let data_storage = Rc::new(RefCell::new(SavedDataStorage::new(world_dir.as_ref().map(WorldDir::data_folder))));
        let tick_rate_manager = ServerTickRateManager::default();
        let main_thread_wakeup = levels[0].chunks.main_thread_wakeup();
        for level in &mut levels {
            level.chunks.set_main_thread_wakeup(main_thread_wakeup.clone());
            level.game_rules = game_rules.clone();
            level.enabled_features = data_configuration.enabled_features;
            level.chunks.game_rules = game_rules.clone();
            level.weather = weather.clone();
            level.default_game_type = default_game_type.clone();
            level.scoreboard = scoreboard.clone();
            level.chunks.scoreboard = scoreboard.clone();
            level.command_storage = command_storage.clone();
            level.random_sequences = random_sequences.clone();
            level.server_data_storage = data_storage.clone();
            level.tick_rate_manager = tick_rate_manager.shared();
            level.prepare_weather();
            level.sync_clocks(&clocks);
            level.overworld_clock_time = clocks.clocks[ServerClockManager::id_of(OVERWORLD).unwrap()].total_ticks;
            level.update_sky_brightness();
            level.game_time = game_time;
            level.chunks.game_time = game_time;
        }
        let name_to_id_cache = CachedUserNameToIdResolver::load(config.server_dir.clone());
        name_to_id_cache.save();
        let lists = match player_lists {
            Some(lists) => lists,
            None => PlayerLists::load(config.server_dir.clone()).map_err(|throwable| std::io::Error::other(StartupFailure { throwable, saving_players: false, saved_levels: Vec::new(), snapshot: None }))?,
        };
        let (config_replies_to_status, config_accepts_transfers) = (config.replies_to_status, config.accepts_transfers);
        let mut server = Server {
            config,
            settings: None,
            levels,
            player_list: Vec::new(),
            next_player_list_order: 0,
            player_names_array: Arc::default(),
            connections: HashMap::new(),
            lost_connections: HashMap::new(),
            clocks,
            game_rules,
            weather,
            scoreboard,
            command_storage,
            random_sequences,
            data_storage,
            ops: lists.ops,
            bans: lists.bans,
            ip_bans: lists.ip_bans,
            whitelist: lists.whitelist,
            empty_ticks: 0,
            send_all_player_info_in: 0,
            seed,
            tick_count: 0,
            debug_command_profiler: None,
            game_test_ticker: Default::default(),
            debug_command_profiler_delay_start: false,
            ticks_until_autosave: AUTOSAVE_INTERVAL,
            world_gen_settings_dirty: true,
            tick_rate_manager,
            tick_times_nanos: [0; 100],
            aggregated_tick_times_nanos: 0,
            pending_logins: Vec::new(),
            login_disconnections_to_report: 0,
            prepare_spawn_tasks: Vec::new(),
            tasks: VecDeque::new(),
            console_inputs: Vec::new(),
            rcon_buffer: std::cell::RefCell::new(String::new()),
            command_origin: command_origin::CommandOrigin::default(),
            command_block_messages: std::cell::RefCell::new(Vec::new()),
            random: LegacyRandomSource::create(),
            status: Arc::default(),
            last_status: None,
            status_icon: None,
            session_id: None,
            #[cfg(test)]
            console_log: Default::default(),
            effective_respawn_data: spawn.clone(),
            spawn,
            default_game_type,
            difficulty,
            difficulty_locked,
            hardcore,
            generate_structures,
            world_gen_settings: gen_settings,
            world_gen_settings_lifecycle,
            special_world_property,
            bonus_chest,
            server_brands,
            was_modded,
            level_name,
            allow_commands,
            singleplayer_uuid,
            scheduled_events,
            name_to_id_cache: Arc::new(name_to_id_cache),
            profile_resolver: Arc::default(),
            task_sender: None,
            custom_boss_events,
            stopwatches,
            services: Services::offline(),
            functions: ServerFunctionManager::new(data_packs.function_library),
            pack_repository,
            data_configuration,
            scoreboard_save_data,
            notification_manager: NotificationManager::default(),
            server_activity_monitor: Arc::new(ServerActivityMonitor::new(NotificationManager::default(), 30)),
            listener_settings: Arc::new(ListenerSettings::new(config_replies_to_status, config_accepts_transfers)),
            world_dir,
            chunk_io_error_reporter,
            stopping: false,
            ready: false,
        };
        server.name_to_id_cache.set_resolve_offline_users(!server.config.online_mode);
        server.attach_user_lookup();
        server.set_difficulty(server.config.difficulty, true);
        server.prepare_levels(&mut load_listener);
        server.update_effective_respawn_data();
        Ok(server)
    }

    fn attach_user_lookup(&mut self) {
        let lookup = players::old_users_converter::UserLookup { name_to_id_cache: self.name_to_id_cache.clone(), services: self.services.clone(), uses_authentication: self.config.online_mode };
        for level in &mut self.levels {
            level.user_lookup = Some(lookup.clone());
        }
    }

    pub fn watchdog_snapshot(&self) -> dedicated::server_watchdog::ServerSnapshot {
        let players: Vec<String> = self
            .players()
            .map(|p| crate::world::entity::java_entity_to_string("ServerPlayer", &p.entity, &p.profile.name, &self.level_name))
            .collect();
        let level_stats = self.levels.iter().map(|level| format!("{}: {}", level.dimension, level.get_watchdog_stats())).collect::<Vec<_>>().join(",\n");
        dedicated::server_watchdog::ServerSnapshot {
            player_count: Some(format!("{} / {}; [{}]", players.len(), self.config.max_players, players.join(", "))),
            random_tick_rate: self.game_rules.get_int(wafer_data::game_rule::RANDOM_TICK_SPEED).to_string(),
            level_stats,
            world_seed: self.seed,
            world_gen_settings_lifecycle: self.world_gen_settings_lifecycle,
            ..system_report_snapshot(&self.pack_repository, self.data_configuration.enabled_features, self.seed, Some(self.chunk_io_error_reporter.clone()))
        }
    }

    fn prepare_levels(&mut self, listener: &mut level::progress::LoggingLevelLoadListener) {
        const PREPARE_LEVELS_DEFAULT_DELAY: Duration = Duration::from_millis(10);
        let mut counter = level::progress::ChunkLoadCounter::default();
        for (index, level) in self.levels.iter_mut().enumerate() {
            counter.track(index, &mut level.chunks, ChunkCache::activate_all_deactivated_tickets);
        }
        listener.start(level::progress::Stage::LoadInitialChunks, counter.total_chunks());
        for level in &mut self.levels {
            level.preparing_levels = true;
        }
        loop {
            let ready = counter.ready_chunks(&self.levels);
            listener.update(level::progress::Stage::LoadInitialChunks, ready, counter.total_chunks());
            self.wait_until_next_tick(Instant::now() + PREPARE_LEVELS_DEFAULT_DELAY);
            if counter.pending_chunks(&self.levels) == 0 {
                break;
            }
        }
        for level in &mut self.levels {
            level.preparing_levels = false;
        }
        listener.finish(level::progress::Stage::LoadInitialChunks);
        self.update_mob_spawning_flags();
    }

    fn wait_until_next_tick(&mut self, deadline: Instant) {
        let wakeup = self.overworld().chunks.main_thread_wakeup();
        while let Some(wait) = deadline.checked_duration_since(Instant::now()).filter(|w| !w.is_zero()) {
            let seen = wakeup.generation();
            if !self.poll_task() {
                wakeup.wait(seen, wait);
            }
        }
    }

    fn poll_task(&mut self) -> bool {
        crate::crash_report::throw_delayed_exception();
        let mut did_something = false;
        for level in &mut self.levels {
            did_something |= level.poll_chunk_task();
        }
        did_something
    }

    pub fn overworld(&self) -> &ServerLevel {
        &self.levels[0]
    }

    pub fn overworld_mut(&mut self) -> &mut ServerLevel {
        &mut self.levels[0]
    }

    pub fn get_level(&self, dimension: &str) -> Option<usize> {
        self.levels.iter().position(|level| level.dimension == dimension)
    }

    pub fn players(&self) -> impl Iterator<Item = &ServerPlayer> {
        self.player_list.iter().map(|&id| self.player(id))
    }

    pub fn take_player_list_order(&mut self) -> u64 {
        let order = self.next_player_list_order;
        self.next_player_list_order += 1;
        order
    }

    pub fn player_ids(&self) -> Vec<EntityId> {
        self.player_list.clone()
    }

    pub fn player_count(&self) -> usize {
        self.player_list.len()
    }

    pub fn get_player(&self, id: EntityId) -> Option<&ServerPlayer> {
        self.levels.iter().find_map(|level| level.players.get(&id))
    }

    pub fn get_player_mut(&mut self, id: EntityId) -> Option<&mut ServerPlayer> {
        self.levels.iter_mut().find_map(|level| level.players.get_mut(&id))
    }

    pub fn player(&self, id: EntityId) -> &ServerPlayer {
        self.get_player(id).expect("player on this server")
    }

    pub fn player_mut(&mut self, id: EntityId) -> &mut ServerPlayer {
        self.get_player_mut(id).expect("player on this server")
    }

    pub fn has_player(&self, id: EntityId) -> bool {
        self.levels.iter().any(|level| level.players.contains_key(&id))
    }

    pub fn level_of(&self, entity: EntityId) -> Option<usize> {
        self.levels.iter().position(|level| level.players.contains_key(&entity) || level.entities.contains_key(&entity) || level.dragon_parts.contains_key(entity))
    }

    pub fn entity_level(&self, entity: EntityId) -> &ServerLevel {
        &self.levels[self.level_of(entity).expect("entity on this server")]
    }

    pub fn entity_level_mut(&mut self, entity: EntityId) -> &mut ServerLevel {
        let index = self.level_of(entity).expect("entity on this server");
        &mut self.levels[index]
    }

    pub fn lend_level(&mut self, index: usize) -> level::lent_levels::LentLevels<'_> {
        level::lent_levels::LentLevels::lend(&mut self.levels, index)
    }

    pub fn lend_entity_level(&mut self, entity: EntityId) -> level::lent_levels::LentLevels<'_> {
        let index = self.level_of(entity).expect("entity on this server");
        self.lend_level(index)
    }

    pub fn set_difficulty(&mut self, difficulty: Difficulty, ignore_lock: bool) {
        if ignore_lock || !self.difficulty_locked {
            self.difficulty = if self.hardcore { Difficulty::Hard } else { difficulty };
            for level in &mut self.levels {
                level.difficulty = self.difficulty;
                level.chunks.difficulty = self.difficulty;
            }
            self.update_mob_spawning_flags();
            for player in self.players() {
                self.send_difficulty_update(&player.connection);
            }
        }
    }

    pub fn update_mob_spawning_flags(&mut self) {
        for level in &mut self.levels {
            level.chunks.spawn_enemies = level.is_spawning_monsters();
        }
    }

    pub fn set_difficulty_locked(&mut self, locked: bool) {
        self.difficulty_locked = locked;
        for player in self.players() {
            self.send_difficulty_update(&player.connection);
        }
    }

    fn send_difficulty_update(&self, connection: &Outbound) {
        connection.send(clientbound::CHANGE_DIFFICULTY, |p| {
            p.varint(self.difficulty.id());
            p.bool(self.difficulty_locked);
        });
    }

    pub fn forced_game_type(&self) -> Option<GameType> {
        self.config.force_game_mode.then_some(self.default_game_type.get())
    }

    pub fn enforce_game_type_for_players(&mut self, game_type: Option<GameType>) -> i32 {
        let Some(game_type) = game_type else { return 0 };
        let ids = self.player_ids();
        ids.into_iter().filter(|&id| self.entity_level_mut(id).set_game_mode(id, game_type)).count() as i32
    }

    fn calculate_game_mode_for_new_player(&self, loaded: Option<GameType>) -> GameType {
        self.forced_game_type().unwrap_or(loaded.unwrap_or(self.default_game_type.get()))
    }

    pub fn set_respawn_data(&mut self, data: RespawnData) {
        if self.spawn == data {
            return;
        }
        for player in self.players() {
            player.connection.send(clientbound::SET_DEFAULT_SPAWN_POSITION, |p| write_respawn_data(p, &data));
        }
        self.spawn = data;
        self.update_effective_respawn_data();
    }

    fn update_effective_respawn_data(&mut self) {
        let respawn_level = self.find_respawn_dimension();
        self.effective_respawn_data = self.levels[respawn_level].get_world_border_adjusted_respawn_data(self.spawn.clone());
        for level in &mut self.levels {
            level.respawn_data = self.effective_respawn_data.clone();
        }
    }

    pub fn find_respawn_dimension(&self) -> usize {
        self.get_level(&self.spawn.dimension).unwrap_or(0)
    }

    pub fn respawn_data(&self) -> &RespawnData {
        &self.effective_respawn_data
    }

    fn write_spawn(&self, p: &mut wafer_protocol::Writer) {
        write_respawn_data(p, &self.effective_respawn_data);
    }

    pub fn set_weather_parameters(&mut self, clear_time: i32, rain_time: i32, raining: bool, thundering: bool) {
        let mut weather = self.weather.get();
        weather.clear_weather_time = clear_time;
        weather.rain_time = rain_time;
        weather.thunder_time = rain_time;
        weather.raining = raining;
        weather.thundering = thundering;
        self.weather.set(weather);
    }

    pub fn save_everything(&mut self, silent: bool, flush: bool, force: bool) -> bool {
        self.flush_other_level_ender_pearls();
        for player in self.players() {
            self.save_player(player);
        }
        let result = self.save_all_chunks(silent, flush, force);
        self.chunk_io_error_reporter.warn_on_low_disk_space();
        result
    }

    pub fn save_all_chunks(&mut self, silent: bool, flush: bool, force: bool) -> bool {
        self.notification_manager.server_save_started();
        let saved_chunks = self.save_all_level_chunks(silent, flush, force);
        self.notification_manager.server_save_completed();
        saved_chunks
    }

    fn save_all_level_chunks(&mut self, silent: bool, flush: bool, force: bool) -> bool {
        let Some(dir) = &self.world_dir else { return false };
        self.scoreboard.borrow_mut().store_to_save_data_if_dirty(&mut self.scoreboard_save_data);
        for level in &mut self.levels {
            if !silent {
                info!("Saving chunks for level 'ServerLevel[{}]'/{}", self.level_name, level.dimension);
            }
            let no_save = level.no_save && !force;
            level.save(dir, flush, no_save);
        }
        let data = WorldData {
            game_time: self.overworld().game_time,
            spawn: self.spawn.clone(),
            game_type: self.default_game_type.get(),
            difficulty: self.difficulty,
            difficulty_locked: self.difficulty_locked,
            hardcore: self.hardcore,
            clocks: self.clocks.to_nbt(),
            weather: self.weather.get(),
            special_world_property: self.special_world_property,
            world_gen_settings_lifecycle: self.world_gen_settings_lifecycle,
            initialized: true,
            server_brands: self.server_brands.clone(),
            was_modded: self.was_modded,
            game_rules: self.game_rules.to_nbt(),
            data_configuration: self.data_configuration.clone(),
            level_name: self.level_name.clone(),
            allow_commands: self.allow_commands,
            singleplayer_uuid: self.singleplayer_uuid.clone(),
        };
        let dirty = DirtySavedData {
            world_gen_settings: self.world_gen_settings_dirty,
            world_clocks: self.clocks.dirty,
            weather: self.weather.is_dirty(),
            game_rules: self.game_rules.is_dirty(),
        };
        self.world_gen_settings_dirty = false;
        self.clocks.dirty = false;
        self.weather.set_dirty(false);
        self.game_rules.set_dirty(false);
        if data.save(dir, &self.world_gen_settings, dirty).is_err() {
            error!("Failed to save level {}", dir.root.display());
        }
        log_save_failure("command_storage.dat", self.command_storage.borrow_mut().save());
        log_save_failure("random_sequences.dat", self.random_sequences.borrow_mut().save());
        log_save_failure("scheduled_events.dat", self.scheduled_events.save());
        log_save_failure("custom_boss_events.dat", self.custom_boss_events.save());
        log_save_failure("stopwatches.dat", self.stopwatches.save());
        log_save_failure("scoreboard.dat", self.scoreboard_save_data.save());
        self.data_storage.borrow_mut().save();
        true
    }

    fn refresh_spawn_protection(&mut self) {
        let ops: Vec<(u128, i32)> = self.ops.entries().into_iter().map(|e| (e.user.id, e.level)).collect();
        for level in &mut self.levels {
            let protection = &mut level.spawn_protection;
            protection.radius = self.config.spawn_protection;
            protection.dimension = self.effective_respawn_data.dimension.clone();
            protection.spawn = self.effective_respawn_data.pos;
            protection.ops.clone_from(&ops);
        }
    }

    fn auto_save(&mut self) {
        self.ticks_until_autosave = self.compute_next_autosave_interval();
        self.save_everything(true, false, false);
    }

    pub fn set_auto_save(&mut self, enable: bool) -> bool {
        let mut success = false;
        for level in &mut self.levels {
            if level.no_save == enable {
                level.no_save = !enable;
                success = true;
            }
        }
        success
    }

    pub fn halt(&mut self) {
        self.stopping = true;
    }

    pub fn reload_resources<S: AsRef<str>>(&mut self, packs_to_enable: &[S], invoker: &[&'static str]) -> Result<(), crate::util::java_throwable::JavaThrowable> {
        let packs_to_load = packs_to_enable.iter().filter_map(|id| self.pack_repository.pack(id.as_ref())).map(|pack| pack.open()).collect();
        let resource_manager = MultiPackResourceManager::new(packs_to_load);
        let postponed_tags = wafer_data::prepare_tags(crate::tags::load_tags_for_existing_registries(&resource_manager, crate::tags::ExistingTagLoading::Reload(invoker)));
        let mut resources = ReloadableServerResources::load_resources(&resource_manager, postponed_tags, self.config.function_permission_level)?;
        self.pack_repository.set_selected(packs_to_enable);
        self.data_configuration = WorldDataConfiguration::new(get_selected_packs(&self.pack_repository, true), self.data_configuration.enabled_features);
        resources.update_components_and_static_registry_tags();
        resources.finalize_recipe_loading();
        crate::world::level::levelgen::structure::templatesystem::structure_template_manager::on_resource_manager_reload(&resource_manager);
        self.flush_other_level_ender_pearls();
        for player in self.players() {
            self.save_player(player);
        }
        self.reload_player_advancements();
        for player in self.players() {
            player.connection.send(clientbound::UPDATE_TAGS, crate::network::configuration::write_tags);
        }
        for player in self.players() {
            player.connection.send(clientbound::UPDATE_RECIPES, crate::world::item::crafting::recipe_manager::write_update_recipes);
            crate::stats::server_recipe_book::send_initial_recipe_book(&player.connection, &player.recipe_book);
        }
        self.functions.replace_library(resources.function_library);
        Ok(())
    }

    fn stop_server(&mut self) {
        self.notification_manager.server_shutting_down();
        info!("Stopping server");
        info!("Saving players");
        self.flush_other_level_ender_pearls();
        for player in self.players() {
            self.save_player(player);
        }
        self.remove_all();
        info!("Saving worlds");
        for level in &mut self.levels {
            level.no_save = false;
        }
        while self.levels.iter().any(|level| level.chunks.has_work()) {
            let next_tick = Instant::now() + Duration::from_millis(1);
            for level in &mut self.levels {
                level.chunks.deactivate_tickets_on_closing();
                level.tick_chunk_source_without_chunks();
            }
            self.wait_until_next_tick(next_tick);
        }
        self.save_all_chunks(false, true, false);
        for level in &mut self.levels {
            level.close();
        }
    }

    pub fn modify_clock(&mut self, clock: usize) {
        self.clocks.dirty = true;
        let advance_time = self.game_rules.get_bool(game_rule::ADVANCE_TIME);
        let game_time = self.overworld().game_time;
        for p in self.players() {
            p.connection.send(clientbound::SET_TIME, |w| self.clocks.write_set_time(w, game_time, &[clock], advance_time));
        }
        for level in &mut self.levels {
            level.sync_clocks(&self.clocks);
        }
    }

    pub fn set_game_rule(&mut self, rule: wafer_data::GameRule, value: i32) {
        self.game_rules.set(rule, value);
        self.notification_manager.on_game_rule_changed(rule, value);
        if rule == game_rule::REDUCED_DEBUG_INFO {
            let event = if value != 0 { 22 } else { 23 };
            for p in self.players() {
                p.connection.send(clientbound::ENTITY_EVENT, |w| {
                    w.i32(p.entity.id);
                    w.u8(event);
                });
            }
        } else if rule == game_rule::LIMITED_CRAFTING || rule == game_rule::IMMEDIATE_RESPAWN {
            let event = if rule == game_rule::LIMITED_CRAFTING { GAME_EVENT_LIMITED_CRAFTING } else { GAME_EVENT_IMMEDIATE_RESPAWN };
            for p in self.players() {
                p.connection.send(clientbound::GAME_EVENT, |w| {
                    w.u8(event);
                    w.f32(if value != 0 { 1.0 } else { 0.0 });
                });
            }
        } else if rule == game_rule::LOCATOR_BAR {
            for level in &self.levels {
                let mut waypoint_manager = level.get_waypoint_manager();
                if value != 0 {
                    for &player in level.players.keys() {
                        waypoint_manager.update_player(level, player);
                    }
                } else {
                    waypoint_manager.break_all_connections(level);
                }
            }
        } else if rule == game_rule::SPAWN_MONSTERS {
            self.update_mob_spawning_flags();
        } else if rule == game_rule::ADVANCE_TIME {
            for p in self.players() {
                p.connection.send(clientbound::SET_TIME, |w| self.full_time_sync(w));
            }
        }
    }

    fn save_player(&self, player: &ServerPlayer) {
        let nbt = self.save_player_with_ender_pearls(player.entity.id);
        self.save_player_data(&nbt, player);
    }

    fn save_player_with_ender_pearls(&self, id: EntityId) -> Nbt {
        self.entity_level(id).save_player_with_ender_pearls(id, &|pearl| self.level_of(pearl).map(|level| &self.levels[level]))
    }

    fn save_player_data(&self, nbt: &Nbt, player: &ServerPlayer) {
        let Some(dir) = &self.world_dir else { return };
        crate::world::level::storage::player_data_storage::PlayerDataStorage::new(dir).save(nbt, player.profile.uuid, &player.profile.name);
        player.stats.save();
        player.advancements.save();
    }

    fn handle(&mut self, event: Event) {
        match event {
            Event::Join { conn, profile, cookie, out, address } => {
                let (failed_profile, failed_out) = (profile.clone(), out.clone());
                if let Err(exception) = crate::util::catch_exception(|| self.place_new_player(conn, profile, cookie, out, address)) {
                    error!("Couldn't place player in world\n{exception}");
                    disconnect_invalid_player_data(&failed_profile, &failed_out);
                }
            }
            Event::Packet { conn, packet } => {
                let Some(&id) = self.connections.get(&conn) else { return };
                self.player_mut(id).received_packets += 1;
                let result = match packet.id {
                    serverbound::CHAT => self.handle_chat(id, &packet),
                    serverbound::CHAT_COMMAND => self.handle_chat_command(id, &packet),
                    serverbound::CHAT_COMMAND_SIGNED => self.handle_signed_chat_command(id, &packet),
                    serverbound::CHAT_ACK => self.handle_chat_ack(id, &packet),
                    serverbound::CHAT_SESSION_UPDATE => self.handle_chat_session_update(id, &packet),
                    serverbound::COMMAND_SUGGESTION => self.handle_custom_command_suggestions(id, &packet),
                    serverbound::CLIENT_COMMAND => self.handle_client_command(id, &packet),
                    serverbound::CLIENT_INFORMATION => self.handle_client_information(id, &packet),
                    serverbound::SET_GAME_RULE => self.handle_set_game_rule(id, &packet),
                    serverbound::CHANGE_DIFFICULTY => self.handle_change_difficulty(id, &packet),
                    serverbound::LOCK_DIFFICULTY => self.handle_lock_difficulty(id, &packet),
                    serverbound::TEST_INSTANCE_BLOCK_ACTION => self.handle_test_instance_block_action(id, &packet),
                    serverbound::ENTITY_TAG_QUERY => self.handle_entity_tag_query(id, &packet),
                    serverbound::BLOCK_ENTITY_TAG_QUERY => self.handle_block_entity_tag_query(id, &packet),
                    serverbound::SPECTATOR_ACTION => self.handle_spectator_action(id, &packet),
                    serverbound::TELEPORT_TO_ENTITY => self.handle_teleport_to_entity_packet(id, &packet),
                    serverbound::CHANGE_GAME_MODE => self.handle_change_game_mode(id, &packet),
                    serverbound::PICK_ITEM_FROM_ENTITY => self.handle_pick_item_from_entity(id, &packet),
                    serverbound::RESOURCE_PACK => self.handle_resource_pack_response(id, &packet),
                    _ => self.entity_level_mut(id).handle_game_packet(id, &packet),
                };
                if result.is_err() {
                    self.player_mut(id).disconnect("multiplayer.disconnect.invalid_packet");
                }
                if let Some(level) = self.level_of(id) {
                    self.open_pending_books(level);
                }
                self.entity_level_mut(id).entity_moved(id);
                self.send_global_level_events();
                if self.player(id).disconnected {
                    self.remove_player(conn, None);
                }
            }
            Event::Left { conn, reason } => {
                if self.connections.contains_key(&conn) {
                    self.lost_connections.entry(conn).or_insert(reason);
                }
            }
            Event::Verify { profile, address, user_name, reply } => self.pending_logins.push(PendingLogin { profile, address, user_name, reply, waiting_for_dupe: false }),
            Event::PrepareSpawn { uuid, name, reply } => self.start_prepare_spawn(uuid, &name, reply),
            Event::PrepareSpawnClosed { uuid } => self.prepare_spawn_tasks.retain(|t| t.uuid != uuid),
            Event::Console(line, has_level) => self.console_inputs.push((line, CommandSourceStack::server(self), has_level)),
            Event::Halt => self.halt(),
            Event::Rcon { command, reply } => self.tasks.push_back(Task::Rcon { command, reply }),
        }
    }

    fn full_time_sync(&self, p: &mut wafer_protocol::Writer) {
        self.clocks.write_set_time(p, self.overworld().game_time, &self.clocks.all(), self.game_rules.get_bool(game_rule::ADVANCE_TIME));
    }

    fn place_new_player(&mut self, conn: ConnId, profile: Profile, cookie: CommonListenerCookie, out: Outbound, address: std::net::SocketAddr) {
        let old_name = self.name_to_id_cache.get_by_id(profile.uuid).map_or_else(|| profile.name.clone(), |p| p.name);
        let mut player = ServerPlayer::new(profile, cookie.client_information, out, self.effective_respawn_data.pos);
        player.stats = self.get_player_stats(&player.profile);
        player.advancements = self.get_player_advancements(player.profile.uuid);
        player.latency = cookie.latency;
        player.chat = ChatState::new(player.profile.uuid, &self.config);
        player.set_game_mode_for_player(self.calculate_game_mode_for_new_player(None), None);
        let mut collector = crate::world::level::storage::tag_value_input::ScopedCollector::new_empty(network::prepare_spawn_task::PREPARE_SPAWN_TASK_LOGGER);
        let saved = self.load_player_data(&player.profile.name, player.profile.uuid);
        let prepared = self.prepare_spawn_tasks.iter().position(|t| t.uuid == player.profile.uuid && t.reply.is_none());
        let (level_index, position, y_rot, x_rot) = match prepared {
            Some(index) => {
                let task = self.prepare_spawn_tasks.swap_remove(index);
                self.levels[task.level].wait_for_entities_around(BlockPos::containing(task.position.x, task.position.y, task.position.z).chunk(), 3);
                (task.level, task.position, task.angle.0, task.angle.1)
            }
            None => self.spawn_position(saved.as_ref()),
        };
        if let Some(nbt) = &saved {
            if let Err(cause) = player.load_nbt(nbt) {
                error!("Couldn't place player in world\n{}", crate::world::entity::entity_save::player_loading_exception(cause).print_stack_trace().trim_end());
                disconnect_invalid_player_data(&player.profile, &player.connection);
                return;
            }
            player.entity.load_team(nbt, &self.scoreboard);
            let level = &mut self.levels[level_index];
            if let Some(bed_position) = ServerPlayer::saved_sleeping_pos(nbt).filter(|&pos| level.has_chunk_at(pos)) {
                let state = crate::world::level::BlockGetter::block_state(level, bed_position);
                if let Some(kind) = crate::world::level::block::abstract_bed_block::kind(state.block()) {
                    level.set_block(bed_position, state.set(wafer_data::properties::OCCUPIED, false), crate::world::level::update_flags::UPDATE_ALL);
                    crate::world::level::block::abstract_bed_block::on_stop_sleeping(kind, level, bed_position);
                }
            }
            let (game_type, previous) = player.loaded_game_types;
            player.set_game_mode_for_player(self.calculate_game_mode_for_new_player(game_type), previous);
        }
        self.name_to_id_cache.add(NameAndId { id: player.profile.uuid, name: player.profile.name.clone() });
        player.entity.set_pos(position);
        player.entity.set_y_rot(y_rot);
        player.entity.set_x_rot(x_rot);
        let position = player.entity.position();
        info!(
            "{}[{}] logged in with entity id {} at ({}, {}, {})",
            player.profile.name,
            crate::network::server_connection_listener::loggable_address(address, self.config.log_ips),
            player.entity.id,
            crate::nbt::java_double_to_string(position.x),
            crate::nbt::java_double_to_string(position.y),
            crate::nbt::java_double_to_string(position.z)
        );
        player.address = Some(address);
        let config = &self.config;
        let level = &self.levels[level_index];
        let out = player.connection.clone();
        let id = player.entity.id;
        out.send(clientbound::LOGIN, |p| {
            p.i32(id);
            p.bool(self.hardcore);
            p.varint(self.levels.len() as i32);
            for level in &self.levels {
                p.string(level.dimension);
            }
            p.varint(config.max_players as i32);
            p.varint(config.view_distance);
            p.varint(config.simulation_distance);
            p.bool(false);
            p.bool(true);
            p.bool(false);
            self.create_common_spawn_info(p, level_index, &player);
            p.bool(config.online_mode);
            p.bool(self.enforce_secure_profile());
        });
        self.send_difficulty_update(&out);
        out.send(clientbound::PLAYER_ABILITIES, |p| player.abilities.write(p));
        out.send(clientbound::SET_HELD_SLOT, |p| p.varint(player.inventory.selected as i32));
        out.send(clientbound::UPDATE_RECIPES, crate::world::item::crafting::recipe_manager::write_update_recipes);
        let permission = self.profile_permission_level(player.profile.uuid);
        out.send(clientbound::ENTITY_EVENT, |p| {
            p.i32(id);
            p.u8(EVENT_PERMISSION_LEVEL_ALL + permission as u8);
        });
        out.send(clientbound::COMMANDS, |p| crate::commands::write_commands_packet(p, &CommandSourceStack::for_new_player(self, level_index, &player)));
        player.stats.mark_all_dirty();
        crate::stats::server_recipe_book::send_initial_recipe_book(&out, &player.recipe_book);
        level.scoreboard.borrow().update_entire_scoreboard(&out);
        let (pos, y_rot, x_rot) = (player.entity.position(), player.entity.y_rot, player.entity.x_rot);
        player.teleport(pos, y_rot, x_rot);
        if self.status.read().unwrap().is_some() && !cookie.transferred {
            out.send(clientbound::SERVER_DATA, |p| {
                text(config.motd.clone()).write(p);
                p.bool(self.status_icon.is_some());
                if let Some(icon) = &self.status_icon {
                    p.varint(icon.icon_bytes.len() as i32);
                    p.bytes(&icon.icon_bytes);
                }
            });
        }
        out.send(clientbound::PLAYER_INFO_UPDATE, |p| {
            p.u8(0xFF);
            p.varint(self.player_count() as i32);
            self.players().for_each(|o| o.write_info(p));
        });
        out.send(clientbound::PLAYER_INFO_UPDATE, |p| {
            p.u8(0xFF);
            p.varint(1);
            player.write_info(p);
        });
        out.send(clientbound::INITIALIZE_BORDER, |p| write_initialize_border(p, &level.world_border));
        out.send(clientbound::SET_TIME, |p| self.full_time_sync(p));
        out.send(clientbound::SET_DEFAULT_SPAWN_POSITION, |p| self.write_spawn(p));
        level.write_weather_for_joining(&out);
        out.send(clientbound::GAME_EVENT, |p| {
            p.u8(GAME_EVENT_LEVEL_CHUNKS_LOAD_START);
            p.f32(0.0);
        });
        self.update_joining_player_tick_rate(&out);

        self.last_status = None;
        let display_name = level.player_display_name(&player.profile.name, player.profile.uuid);
        let message = if player.profile.name.eq_ignore_ascii_case(&old_name) {
            player_joined_or_left(display_name, true)
        } else {
            Component::translatable_with("multiplayer.player.joined.renamed", vec![display_name.into(), old_name.into()]).with_color(ChatFormatting::Yellow)
        };
        self.send_system_message(&message);
        for other in self.players() {
            other.send_system_message(&message, false);
            other.connection.send(clientbound::PLAYER_INFO_UPDATE, |p| {
                p.u8(0xFF);
                p.varint(1);
                player.write_info(p);
            });
        }
        self.connections.insert(conn, id);
        player.player_list_order = self.take_player_list_order();
        self.player_list.push(id);
        self.scoreboard.borrow_mut().server.add_connection(id, out.clone());
        self.levels[level_index].add_player(player);
        self.update_player_names_array();
        self.custom_boss_events.on_player_connect(&self.levels, id);
        let level = &mut self.levels[level_index];
        level.send_active_player_effects(id);
        let player = level.player_mut(id);
        player.send_post_effects();
        level.init_inventory_menu(id);
        let player = level.player(id);
        let joined = NameAndId { id: player.profile.uuid, name: player.profile.name.clone() };
        self.notification_manager.player_joined(&joined);
        if let Some(nbt) = &saved {
            self.load_and_spawn_ender_pearls(nbt);
            self.levels[level_index].load_and_spawn_parent_vehicle(id, nbt);
        }
        collector.set_root(crate::util::problem_reporter::PathElement::Text(self.levels[level_index].entity_to_string(id)));
    }

    fn tick_player_list(&mut self) {
        self.send_all_player_info_in += 1;
        if self.send_all_player_info_in > 600 {
            let packet = crate::network::connection::encode(clientbound::PLAYER_INFO_UPDATE, |p| {
                p.u8(1 << 4);
                p.varint(self.player_count() as i32);
                for player in self.players() {
                    p.uuid(player.profile.uuid);
                    p.varint(player.latency);
                }
            });
            for player in self.players() {
                player.connection.send_raw(packet.clone());
            }
            self.send_all_player_info_in = 0;
        }
    }

    fn run_all_tasks(&mut self) {
        while let Some(task) = self.tasks.pop_front() {
            match task {
                Task::Chat { player, body, signature } if self.has_player(player) => self.run_chat(player, body, signature),
                Task::Command { player, command } if self.has_player(player) => self.run_unsigned_command(player, &command),
                Task::SignedCommand { player, command, time_stamp, salt, arguments, last_seen } if self.has_player(player) => {
                    self.run_signed_command(player, &command, time_stamp, salt, arguments, last_seen);
                }
                Task::Chained { player, run } if self.has_player(player) => run(self),
                Task::CommandSuggestions { player } if self.has_player(player) => self.try_process_command_suggestions(player),
                Task::Rcon { command, reply } => {
                    self.rcon_buffer.borrow_mut().clear();
                    let source = CommandSourceStack::rcon(self);
                    self.run_from(CommandOrigin::Rcon, |server| crate::commands::perform_prefixed_command(server, &source, &command));
                    let _ = reply.send(self.rcon_buffer.take());
                }
                _ => {}
            }
        }
    }

    fn handle_console_inputs(&mut self) {
        for (line, source, has_level) in std::mem::take(&mut self.console_inputs) {
            if has_level {
                crate::commands::perform_prefixed_command(self, &source, &line);
            } else {
                crate::commands::perform_prefixed_command_without_level(self, &source, &line);
            }
        }
    }

    fn build_server_status(&mut self) {
        let mut status = serde_json::json!({
            "description": serde_json::from_str::<serde_json::Value>(&Component::literal(self.config.motd.as_str()).to_json()).unwrap(),
            "players": self.build_player_status(),
            "version": { "name": wafer_data::VERSION_NAME, "protocol": wafer_data::PROTOCOL_VERSION },
        });
        if let Some(icon) = &self.status_icon {
            status["favicon"] = serde_json::Value::String(icon.encoded());
        }
        if self.enforce_secure_profile() {
            status["enforcesSecureChat"] = serde_json::Value::Bool(true);
        }
        *self.status.write().unwrap() = Some(status.to_string().into());
    }

    fn build_player_status(&mut self) -> serde_json::Value {
        let players: Vec<&ServerPlayer> = self.levels.iter().flat_map(|level| level.players.values()).collect();
        let mut players_json = serde_json::json!({ "max": self.config.max_players, "online": players.len() });
        if self.config.hides_online_players {
            return players_json;
        }
        let sample_size = players.len().min(MAX_STATUS_PLAYER_SAMPLE);
        let offset = mth::next_int(&mut self.random, 0, (players.len() - sample_size) as i32) as usize;
        let mut sample: Vec<serde_json::Value> = players[offset..offset + sample_size]
            .iter()
            .map(|p| {
                let (uuid, name) = if p.client.allows_listing { (p.profile.uuid, p.profile.name.as_str()) } else { (0, "Anonymous Player") };
                serde_json::json!({ "id": format_uuid(uuid), "name": name })
            })
            .collect();
        for i in (2..=sample.len()).rev() {
            let swap_to = self.random.next_int_bounded(i as i32) as usize;
            sample.swap(i - 1, swap_to);
        }
        if !sample.is_empty() {
            players_json["sample"] = serde_json::Value::Array(sample);
        }
        players_json
    }

    pub fn invalidate_status(&mut self) {
        self.last_status = None;
    }

    fn tick_logins(&mut self) {
        for _ in 0..std::mem::take(&mut self.login_disconnections_to_report) {
            warn!("handleDisconnection() called twice");
        }
        for mut login in std::mem::take(&mut self.pending_logins) {
            if !login.waiting_for_dupe {
                if let Some(reason) = self.can_player_login(&login.profile, &login.address) {
                    let reason_text = reason.get_string();
                    info!("Disconnecting {}: {reason_text}", login.user_name);
                    info!("{} lost connection: {reason_text}", login.user_name);
                    let _ = login.reply.send(Verdict::Disconnect(reason));
                    self.login_disconnections_to_report += 1;
                    continue;
                }
                login.waiting_for_dupe = self.disconnect_all_players_with_profile(login.profile.uuid);
            }
            if login.waiting_for_dupe && self.players().any(|p| p.profile.uuid == login.profile.uuid) {
                self.pending_logins.push(login);
                continue;
            }
            let session_id = *self.session_id.get_or_insert_with(|| {
                let mut bytes: [u8; 16] = rand::random();
                bytes[6] = bytes[6] & 0x0F | 0x40;
                bytes[8] = bytes[8] & 0x3F | 0x80;
                u128::from_be_bytes(bytes)
            });
            let _ = login.reply.send(Verdict::Accept { session_id });
        }
        if self.player_count() == 0 && self.pending_logins.is_empty() && self.connections.is_empty() {
            self.session_id = None;
        }
    }

    fn can_player_login(&mut self, profile: &Profile, address: &std::net::SocketAddr) -> Option<Component> {
        let key = format_uuid(profile.uuid);
        if self.bans.contains_key(&key) {
            return Some(match self.bans.get_key(&key) {
                Some(entry) => players::ban_message("multiplayer.disconnect.banned.reason", "multiplayer.disconnect.banned.expiration", &entry.ban),
                None => Component::literal("Internal server error"),
            });
        }
        if !self.is_white_listed(profile.uuid) {
            return Some(Component::translatable("multiplayer.disconnect.not_whitelisted"));
        }
        let ip = players::ip_from_address(address);
        if self.ip_bans.contains_key(&ip) {
            return Some(match self.ip_bans.get_key(&ip) {
                Some(entry) => players::ban_message("multiplayer.disconnect.banned_ip.reason", "multiplayer.disconnect.banned_ip.expiration", &entry.ban),
                None => Component::literal("Internal server error"),
            });
        }
        let bypass = self.ops.get(profile.uuid).is_some_and(|e| e.bypasses_player_limit);
        (self.player_count() >= self.config.max_players as usize && !bypass).then(|| Component::translatable("multiplayer.disconnect.server_full"))
    }

    pub fn enforce_secure_profile(&self) -> bool {
        self.config.enforce_secure_profile && self.config.online_mode && self.services.can_validate_profile_keys()
    }

    pub fn set_player_idle_timeout(&mut self, minutes: i32) {
        self.config.player_idle_timeout = minutes;
        if let Some(settings) = &mut self.settings {
            settings.update("player-idle-timeout", minutes.to_string());
        }
    }

    pub(crate) fn kick_if_idle(&mut self, id: EntityId) -> bool {
        let timeout = self.config.player_idle_timeout;
        let player = self.player_mut(id);
        if timeout > 0 && player.last_action_time.elapsed() > Duration::from_secs(timeout as u64 * 60) && !player.won_game {
            player.disconnect("multiplayer.disconnect.idling");
            return true;
        }
        false
    }

    fn tick_second(&mut self, id: EntityId) -> bool {
        let rate_limit_packets_per_second = self.config.rate_limit_packets_per_second;
        let player = self.player_mut(id);
        let tick_count = player.connection_tick_count;
        player.connection_tick_count = tick_count.wrapping_add(1);
        if tick_count % 20 != 0 {
            return false;
        }
        let received = player.received_packets as f32;
        player.average_received_packets = received + 0.75 * (player.average_received_packets - received);
        player.received_packets = 0;
        if rate_limit_packets_per_second > 0 && player.average_received_packets > rate_limit_packets_per_second as f32 {
            warn!("Player exceeded rate-limit (sent {} packets per second)", crate::nbt::java_float_to_string(player.average_received_packets));
            player.disconnect("disconnect.exceeded_packet_rate");
            return true;
        }
        false
    }

    pub fn set_using_whitelist(&mut self, enabled: bool) {
        self.config.white_list = enabled;
        if let Some(settings) = &mut self.settings {
            settings.update("white-list", enabled.to_string());
        }
    }

    pub fn kick_unlisted_players(&mut self) {
        if self.config.enforce_whitelist && self.config.white_list {
            let unlisted: Vec<EntityId> =
                self.players().filter(|p| !self.whitelist.contains_key(&format_uuid(p.profile.uuid))).map(|p| p.entity.id).collect();
            for id in unlisted {
                self.player_mut(id).disconnect("multiplayer.disconnect.not_whitelisted");
            }
        }
    }

    pub fn is_white_listed(&self, uuid: u128) -> bool {
        !self.config.white_list || self.ops.contains(uuid) || self.whitelist.contains_key(&format_uuid(uuid))
    }

    fn disconnect_all_players_with_profile(&mut self, uuid: u128) -> bool {
        let dupes: Vec<EntityId> = self.players().filter(|p| p.profile.uuid == uuid).map(|p| p.entity.id).collect();
        for &id in &dupes {
            self.player_mut(id).disconnect("multiplayer.disconnect.duplicate_login");
        }
        !dupes.is_empty()
    }

    pub fn send_system_message(&self, message: &Component) {
        #[cfg(test)]
        self.console_log.borrow_mut().push(message.get_string());
        info!("System chat: {}", message.get_string());
    }

    pub fn broadcast_system_message(&self, message: &Component, overlay: bool) {
        self.send_system_message(message);
        for player in self.players() {
            self.send_player_system_message(player.entity.id, message, overlay);
        }
    }

    pub fn send_player_system_message(&self, id: EntityId, message: &Component, overlay: bool) {
        let Some(player) = self.get_player(id) else { return };
        if player.client.chat_visibility == ChatVisibility::Hidden && !overlay {
            return;
        }
        player.send_system_message(message, overlay);
    }

    pub fn entity_name(&self, id: EntityId) -> String {
        EntityWorld::entity_name(self, id)
    }

    pub fn entity_display_name(&self, id: EntityId) -> Component {
        EntityWorld::entity_display_name(self, id)
    }

    pub fn profile_permission_level(&self, uuid: u128) -> i32 {
        self.ops.get(uuid).map_or(0, |e| e.level)
    }

    pub fn is_op(&self, uuid: u128) -> bool {
        self.ops.contains(uuid)
    }

    pub fn send_player_permission_level(&self, id: EntityId) {
        let Some(player) = self.get_player(id) else { return };
        let level = self.profile_permission_level(player.profile.uuid);
        player.connection.send(clientbound::ENTITY_EVENT, |p| {
            p.i32(id);
            p.u8(EVENT_PERMISSION_LEVEL_ALL + level as u8);
        });
        let source = CommandSourceStack::player(self, id);
        player.connection.send(clientbound::COMMANDS, |p| crate::commands::write_commands_packet(p, &source));
    }

    fn update_player_names_array(&self) {
        let names: Arc<[String]> = self.players().map(|p| p.profile.name.clone()).collect();
        *self.player_names_array.write().unwrap() = names;
    }

    fn remove_all(&mut self) {
        let mut i = 0;
        while let Some(&id) = self.player_list.get(i) {
            self.player_mut(id).disconnect("multiplayer.disconnect.server_shutdown");
            if let Some(conn) = self.connections.iter().find_map(|(&conn, &player)| (player == id).then_some(conn)) {
                self.remove_player(conn, None);
            }
            i += 1;
        }
    }

    fn remove_player(&mut self, conn: ConnId, reason: Option<Component>) {
        self.lost_connections.remove(&conn);
        let Some(id) = self.connections.remove(&conn) else { return };
        let Some(level_index) = self.level_of(id) else { return };
        let level = &mut self.levels[level_index];
        if level.is_sleeping(id) {
            level.stop_sleep_in_bed(id, true, false);
        }
        level.player_mut(id).disconnected = true;
        level.eject_passengers(id);
        level.remove_player_menus(id);
        level.award_stat(id, crate::stats::stats::LEAVE_GAME, 1);
        self.flush_other_level_ender_pearls();
        let saved = self.save_player_with_ender_pearls(id);
        let level = &mut self.levels[level_index];
        if level.is_passenger(id) {
            let vehicle = level.root_vehicle(id);
            if level.has_exactly_one_player_passenger(vehicle) {
                level.stop_riding(id);
                for entity in level.passengers_and_self(vehicle) {
                    level.set_removed(entity, crate::world::entity::RemovalReason::UnloadedWithPlayer);
                    level.remove_entity(entity);
                }
            }
        }
        level.un_ride(id);
        self.remove_ender_pearls(id);
        let level = &self.levels[level_index];
        let leaving = level.player(id);
        let reason = leaving.disconnect_reason.clone().or_else(|| reason.map(|r| r.get_string())).unwrap_or_else(|| "Disconnected".into());
        info!("{} lost connection: {reason}", leaving.profile.name);
        let message = player_joined_or_left(level.player_display_name(&leaving.profile.name, leaving.profile.uuid), false);
        self.invalidate_status();
        self.send_system_message(&message);
        for other in self.players().filter(|other| other.entity.id != id) {
            other.send_system_message(&message, false);
        }
        let Some(player) = self.levels[level_index].remove_player(id) else { return };
        self.data_storage.borrow().player_removed(id);
        self.player_list.retain(|&p| p != id);
        self.scoreboard.borrow_mut().server.remove_connection(id);
        self.update_player_names_array();
        self.custom_boss_events.on_player_disconnect(id);
        self.save_player_data(&saved, &player);
        self.notification_manager.player_left(&NameAndId { id: player.profile.uuid, name: player.profile.name.clone() });
        for other in self.players() {
            other.connection.send(clientbound::PLAYER_INFO_REMOVE, |p| {
                p.varint(1);
                p.uuid(player.profile.uuid);
            });
        }
    }

    pub fn is_time_profiler_running(&self) -> bool {
        self.debug_command_profiler_delay_start || self.debug_command_profiler.is_some()
    }

    pub fn start_time_profiler(&mut self) {
        self.debug_command_profiler_delay_start = true;
    }

    pub fn stop_time_profiler(&mut self) -> Option<commands::debug_command::TimeProfiler> {
        self.debug_command_profiler.take()
    }

    pub(crate) fn tick(&mut self) {
        let start = Instant::now();
        let empty_tick_threshold = self.config.pause_when_empty_seconds * 20;
        if empty_tick_threshold > 0 {
            if self.player_count() == 0 && !self.tick_rate_manager.is_sprinting() {
                self.empty_ticks += 1;
            } else {
                self.empty_ticks = 0;
            }
            if self.empty_ticks >= empty_tick_threshold {
                if self.empty_ticks == empty_tick_threshold {
                    info!("Server empty for {} seconds, pausing", self.config.pause_when_empty_seconds);
                    self.auto_save();
                }
                self.tick_logins();
                self.tick_prepare_spawns();
                self.handle_console_inputs();
                return;
            }
        }
        self.tick_count += 1;
        self.tick_rate_manager.tick();
        ServerFunctionManager::tick(self);
        if self.tick_rate_manager.runs_normally() {
            self.clocks.tick(self.game_rules.get_bool(game_rule::ADVANCE_TIME));
        }
        let overworld_clock_time = self.clocks.clocks[ServerClockManager::id_of(OVERWORLD).unwrap()].total_ticks;
        for level in &mut self.levels {
            level.sync_clocks(&self.clocks);
            level.overworld_clock_time = overworld_clock_time;
        }
        if self.tick_count % 20 == 0 {
            let game_time = self.overworld().game_time;
            for p in self.players() {
                p.connection.send(clientbound::SET_TIME, |w| {
                    w.i64(game_time);
                    w.varint(0);
                });
            }
        }
        self.update_effective_respawn_data();
        for index in 0..self.levels.len() {
            self.tick_level(index);
        }
        for id in self.player_ids() {
            if !self.lost_connections.is_empty()
                && let Some(conn) = self.connections.iter().find(|&(_, &p)| p == id).map(|(&c, _)| c)
                && let Some(reason) = self.lost_connections.remove(&conn)
            {
                self.remove_player(conn, Some(reason));
                continue;
            }
            if let Some(index) = self.level_of(id) {
                level::lent_levels::LentLevels::lend(&mut self.levels, index).level().tick_connection_start(id);
            }
            if let Some(level) = self.level_of(id) {
                self.handle_pending_spectator_portal_use(level);
                self.handle_pending_portal_teleport(level);
            }
            let connected = match self.level_of(id) {
                Some(index) => level::lent_levels::LentLevels::lend(&mut self.levels, index).level().tick_connection_end(id),
                None => false,
            };
            if connected {
                self.tick_command_suggestions(id);
            }
            if !connected || self.kick_if_idle(id) || self.tick_second(id) {
                let conn = self.connections.iter().find(|&(_, &p)| p == id).map(|(&c, _)| c);
                if let Some(conn) = conn {
                    self.remove_player(conn, None);
                }
            }
        }
        self.tick_logins();
        self.tick_prepare_spawns();
        self.handle_console_inputs();
        self.tick_player_list();
        if self.tick_rate_manager.runs_normally() {
            crate::gametest::framework::game_test_ticker::tick(self);
        }
        self.update_scoreboard_waypoints();
        for id in self.player_ids() {
            self.entity_level_mut(id).send_next_chunks(id);
        }
        self.server_activity_monitor.tick();
        if self.last_status.is_none_or(|t| t.elapsed() >= STATUS_EXPIRE_TIME) {
            self.last_status = Some(Instant::now());
            self.build_server_status();
        }
        self.ticks_until_autosave -= 1;
        if self.ticks_until_autosave <= 0 {
            self.auto_save();
        }
        let tick_time = start.elapsed().as_nanos() as i64;
        let tick_index = (self.tick_count % 100) as usize;
        self.aggregated_tick_times_nanos += tick_time - self.tick_times_nanos[tick_index];
        self.tick_times_nanos[tick_index] = tick_time;
    }

    fn tick_level(&mut self, index: usize) {
        if index != 0 {
            let game_time = self.overworld().game_time;
            let level = &mut self.levels[index];
            level.game_time = game_time;
            level.chunks.game_time = game_time;
        }
        let runs = level::lent_levels::LentLevels::lend(&mut self.levels, index).level().tick_until_sleep();
        let level = &mut self.levels[index];
        if level.should_wake_up_all_players() {
            let default_clock = level.dimension_type().default_clock.and_then(ServerClockManager::id_of);
            if self.game_rules.get_bool(game_rule::ADVANCE_TIME)
                && let Some(clock) = default_clock
            {
                self.clocks.move_to_time_marker(clock, "minecraft:wake_up_from_sleep");
                self.modify_clock(clock);
            }
            self.levels[index].wake_up_all_players_after_clock();
        }
        let level = &mut self.levels[index];
        level.update_sky_brightness();
        if runs && index == 0 {
            let time = level.tick_time();
            TimerQueue::tick(self, time);
        }
        level::lent_levels::LentLevels::lend(&mut self.levels, index).level().begin_block_ticks(runs);
        while let Some(pos) = self.levels[index].run_block_ticks() {
            self.command_block_tick(index, pos);
        }
        level::lent_levels::LentLevels::lend(&mut self.levels, index).level().tick_after_block_ticks(runs);
        while level::lent_levels::LentLevels::lend(&mut self.levels, index).level().tick_entities() {
            self.run_pending_minecart_commands(index);
            self.handle_pending_portal_teleport(index);
            self.handle_pending_ender_pearl_teleport(index);
            self.send_global_level_events();
        }
        level::lent_levels::LentLevels::lend(&mut self.levels, index).level().tick_after_entities();
        self.flush_other_level_ender_pearls();
        self.send_global_level_events();
    }

    pub fn run_level_tasks(&mut self) {
        self.deliver_level_broadcasts();
        for index in 0..self.levels.len() {
            for (function, player, trigger) in std::mem::take(&mut self.levels[index].pending_function_executions) {
                if self.level_of(player).is_none() {
                    continue;
                }
                if let Some(function) = self.functions.get(&function) {
                    let source = CommandSourceStack::player(self, player).with_suppressed_output().with_permission(2);
                    self.run_from(CommandOrigin::AdvancementReward(trigger), |server| ServerFunctionManager::execute(server, &function, &source));
                }
            }
            for (name, entity, position, rotation, from_player_tick) in std::mem::take(&mut self.levels[index].pending_enchantment_functions) {
                match self.functions.get(&name) {
                    Some(function) => {
                        let source = CommandSourceStack::server(self).with_permission(2).with_suppressed_output().with_entity(entity).with_level(self, index).with_position(position).with_rotation(rotation);
                        let origin = if from_player_tick { CommandOrigin::EnchantmentTickPlayer } else { self.command_origin };
                        self.run_from(origin, |server| ServerFunctionManager::execute(server, &function, &source));
                    }
                    None => error!("Enchantment run_function effect failed for non-existent function {name}"),
                }
            }
        }
    }

    pub fn send_global_level_events(&mut self) {
        self.run_level_tasks();
        for index in 0..self.levels.len() {
            if self.levels[index].global_level_events.is_empty() {
                continue;
            }
            for (event, pos, data) in std::mem::take(&mut self.levels[index].global_level_events) {
                for &id in &self.player_list {
                    let Some(level) = self.level_of(id) else { continue };
                    let player = self.levels[level].player(id);
                    let position = player.entity.position();
                    let sound_pos = if level == index {
                        let center_of_block = wafer_core::Vec3::at_center_of(pos);
                        if position.distance_to_sqr(center_of_block) < (32 * 32) as f64 {
                            center_of_block
                        } else {
                            let direction_to_event = center_of_block.subtract(position).normalize();
                            position.add_vec(direction_to_event.scale(32.0))
                        }
                    } else {
                        position
                    };
                    let sound_pos = BlockPos::containing_vec(sound_pos);
                    player.connection.send(clientbound::LEVEL_EVENT, |p| {
                        p.i32(event);
                        p.position(sound_pos.x, sound_pos.y, sound_pos.z);
                        p.i32(data);
                        p.bool(true);
                    });
                }
            }
        }
    }

    pub fn tick_times_nanos(&self) -> &[i64; 100] {
        &self.tick_times_nanos
    }
}

pub fn hashed_seed(seed: i64) -> i64 {
    let hash = Sha256::digest(seed.to_le_bytes());
    i64::from_le_bytes(hash[..8].try_into().unwrap())
}

impl Server {
    fn load_status_icon(&self) -> Option<Favicon> {
        let is_regular_file = |path: &PathBuf| std::fs::metadata(path).is_ok_and(|m| m.is_file());
        let icon_path = self
            .config
            .server_dir
            .as_ref()
            .map(|dir| dir.join("server-icon.png"))
            .filter(is_regular_file)
            .or_else(|| self.world_dir.as_ref().map(WorldDir::icon_file).filter(is_regular_file))?;
        let loaded = std::fs::read(&icon_path).map_err(|e| format!("java.io.IOException: {e}")).and_then(|contents| {
            let png_info = crate::util::png_info::PngInfo::from_bytes(&contents)?;
            if png_info.width == 64 && png_info.height == 64 {
                Ok(Favicon { icon_bytes: contents })
            } else {
                Err(format!(
                    "java.lang.IllegalArgumentException: Invalid world icon size [{}, {}], but expected [64, 64]",
                    png_info.width, png_info.height
                ))
            }
        });
        loaded.inspect_err(|e| error!("Couldn't load server icon\n{e}")).ok()
    }
}

fn write_respawn_data(p: &mut wafer_protocol::Writer, data: &RespawnData) {
    p.string(&data.dimension);
    p.position(data.pos.x, data.pos.y, data.pos.z);
    p.f32(data.yaw);
    p.f32(data.pitch);
}

fn disconnect_invalid_player_data(profile: &Profile, connection: &Outbound) {
    let reason = crate::network::chat::translate("multiplayer.disconnect.invalid_player_data");
    info!("{} ({}) lost connection: {}", profile.name, format_uuid(profile.uuid), reason.get_string());
    connection.send(clientbound::DISCONNECT, |p| reason.write(p));
}

pub(crate) fn log_save_failure(file_name: &str, result: std::io::Result<()>) {
    if result.is_err() {
        error!("Could not save data to {file_name}");
    }
}

pub fn configure_pack_repository(
    pack_repository: &mut PackRepository,
    initial_data_config: &WorldDataConfiguration,
    init_mode: bool,
    safe_mode: bool,
) -> WorldDataConfiguration {
    let data_pack_config = &initial_data_config.data_packs;
    let forced_features = if init_mode { FeatureFlagSet::default() } else { initial_data_config.enabled_features };
    let allowed_features = if init_mode { FeatureFlagSet::ALL_FLAGS } else { initial_data_config.enabled_features };
    pack_repository.reload();
    if safe_mode {
        return configure_repository_with_selection(pack_repository, &[packs::repository::VANILLA_ID.to_string()], forced_features, false);
    }
    let mut selected: Vec<String> = Vec::new();
    for id in &data_pack_config.enabled {
        if pack_repository.is_available(id) {
            if !selected.contains(id) {
                selected.push(id.clone());
            }
        } else {
            warn!("Missing data pack {id}");
        }
    }
    for pack in pack_repository.available_packs() {
        let pack_id = pack.id();
        if data_pack_config.disabled.iter().any(|d| d == pack_id) {
            continue;
        }
        let pack_features = pack.requested_features();
        let is_selected = selected.iter().any(|s| s == pack_id);
        if !is_selected && pack.pack_source().should_add_automatically() {
            if pack_features.is_subset_of(allowed_features) {
                info!("Found new data pack {pack_id}, loading it automatically");
                selected.push(pack_id.to_string());
            } else {
                info!(
                    "Found new data pack {pack_id}, but can't load it due to missing features {}",
                    print_missing_flags(allowed_features, pack_features)
                );
            }
        }
        if is_selected && !pack_features.is_subset_of(allowed_features) {
            warn!(
                "Pack {pack_id} requires features {} that are not enabled for this world, disabling pack.",
                print_missing_flags(allowed_features, pack_features)
            );
            selected.retain(|s| s != pack_id);
        }
    }
    if selected.is_empty() {
        info!("No datapacks selected, forcing vanilla");
        selected.push(packs::repository::VANILLA_ID.to_string());
    }
    configure_repository_with_selection(pack_repository, &selected, forced_features, true)
}

fn configure_repository_with_selection(
    pack_repository: &mut PackRepository,
    selected: &[String],
    forced_features: FeatureFlagSet,
    disable_inactive: bool,
) -> WorldDataConfiguration {
    pack_repository.set_selected(selected);
    enable_forced_feature_packs(pack_repository, forced_features);
    let pack_config = get_selected_packs(pack_repository, disable_inactive);
    let pack_requested_features = pack_repository.requested_feature_flags().join(forced_features);
    WorldDataConfiguration::new(pack_config, pack_requested_features)
}

fn enable_forced_feature_packs(pack_repository: &mut PackRepository, forced_features: FeatureFlagSet) {
    let provided_features = pack_repository.requested_feature_flags();
    let mut missing_features = forced_features.subtract(provided_features);
    if missing_features.is_empty() {
        return;
    }
    let mut selected: Vec<String> = pack_repository.selected_ids().into_iter().map(str::to_string).collect();
    for pack in pack_repository.available_packs() {
        if missing_features.is_empty() {
            break;
        }
        if pack.pack_source() == PackSource::Feature {
            let pack_features = pack.requested_features();
            if !pack_features.is_empty() && pack_features.intersects(missing_features) && pack_features.is_subset_of(forced_features) {
                if selected.iter().any(|s| s == pack.id()) {
                    panic!("Tried to force '{}', but it was already enabled", pack.id());
                }
                selected.push(pack.id().to_string());
                info!("Found feature pack ('{}') for requested feature, forcing to enabled", pack.id());
                missing_features = missing_features.subtract(pack_features);
            }
        }
    }
    pack_repository.set_selected(&selected);
}

pub fn get_selected_packs(pack_repository: &PackRepository, disable_inactive: bool) -> DataPackConfig {
    let selected = pack_repository.selected_ids();
    let disabled = if disable_inactive {
        pack_repository.available_ids().filter(|id| !selected.contains(id)).map(str::to_string).collect()
    } else {
        Vec::new()
    };
    DataPackConfig { enabled: selected.into_iter().map(str::to_string).collect(), disabled }
}

#[allow(clippy::too_many_arguments)]
pub fn run(
    config: ServerConfig,
    settings: Option<dedicated::dedicated_server_settings::DedicatedServerSettings>,
    world: Option<(PackRepository, WorldStem)>,
    services: Services,
    mut events: mpsc::UnboundedReceiver<Event>,
    sender: mpsc::UnboundedSender<Event>,
    runtime: tokio::runtime::Handle,
    listen: crate::Listen,
    console: bool,
    management_server: Option<Arc<ManagementServer>>,
) -> anyhow::Result<()> {
    let (task_sender, tasks) = std::sync::mpsc::channel::<ServerTask>();
    if let Some(management_server) = &management_server {
        management_server.set_server(task_sender.clone());
    }
    let notification_manager = NotificationManager::new(management_server.clone());
    let server_activity_monitor = Arc::new(ServerActivityMonitor::new(notification_manager.clone(), 30));
    let listener_settings = Arc::new(ListenerSettings::new(config.replies_to_status, config.accepts_transfers));
    if console {
        dedicated::start_console_thread(sender.clone());
    }
    info!("Starting minecraft server version {}", wafer_data::VERSION_NAME);
    info!("Loading properties");
    info!("Default game type: {}", config.game_mode.name().to_uppercase());
    let local_address = match &listen {
        crate::Listen::Bind { ip, .. } if !ip.is_empty() => match dedicated::get_by_name(ip) {
            Ok(address) => Some(address),
            Err(throwable) => {
                report_startup_failure(&StartupFailure { throwable, saving_players: false, saved_levels: Vec::new(), snapshot: world_snapshot(&world) });
                return Ok(());
            }
        },
        _ => None,
    };
    info!("Generating keypair");
    let keys = Arc::new(crate::network::encryption::ServerKeyPair::generate()?);
    let listener = match listen {
        crate::Listen::Bind { ip, port } => {
            info!("Starting Minecraft server on {}:{port}", if ip.is_empty() { "*" } else { &ip });
            let _guard = runtime.enter();
            match crate::network::server_connection_listener::start_tcp_server_listener(local_address, port) {
                Ok(listener) => listener,
                Err(e) => {
                    warn!("**** FAILED TO BIND TO PORT!");
                    warn!("The exception was: {}", dedicated::java_bind_exception(&e));
                    warn!("Perhaps a server is already running on that port?");
                    return Ok(());
                }
            }
        }
        crate::Listen::Bound(listener) => {
            let local = listener.local_addr()?;
            info!("Starting Minecraft server on {}:{}", local.ip(), local.port());
            listener
        }
    };
    if !config.online_mode {
        warn!("**** SERVER IS RUNNING IN OFFLINE/INSECURE MODE!");
        warn!("The server will make no attempt to authenticate usernames. Beware.");
        warn!("While this makes the game possible to play without internet access, it also opens up the ability for hackers to connect with any username they choose.");
        warn!("To change this, set \"online-mode\" to \"true\" in the server.properties file.");
    }
    if let Some(server_directory) = &config.server_dir {
        let name_to_id_cache = CachedUserNameToIdResolver::load(Some(server_directory.clone()));
        let old_users_converter = players::old_users_converter::OldUsersConverter {
            server_directory,
            world_directory: config.world.as_deref(),
            services: &services,
            name_to_id_cache: &name_to_id_cache,
            uses_authentication: config.online_mode,
            operator_permission_level: config.op_permission_level,
        };
        match old_users_converter.convert_old_users() {
            Ok(true) => name_to_id_cache.save(),
            Ok(false) => {}
            Err(throwable) => {
                report_startup_failure(&StartupFailure { throwable, saving_players: false, saved_levels: Vec::new(), snapshot: world_snapshot(&world) });
                return Ok(());
            }
        }
        if !old_users_converter.are_old_userlists_removed() {
            let throwable = JavaThrowable::new("java.lang.IllegalStateException: Failed to initialize server", vec![RUN_SERVER_INITIALIZE_FRAME, SPIN_FRAME, THREAD_RUN_FRAME]);
            report_startup_failure(&StartupFailure { throwable, saving_players: false, saved_levels: Vec::new(), snapshot: world_snapshot(&world) });
            return Ok(());
        }
    }
    let port = listener.local_addr()?.port();
    let status = Arc::new(RwLock::new(None));
    let player_names_array: Arc<RwLock<Arc<[String]>>> = Arc::default();
    let context = Arc::new(crate::network::server_connection_listener::ListenerContext {
        config: config.clone(),
        keys,
        services: services.clone(),
        status: status.clone(),
        player_names_array: player_names_array.clone(),
        events: sender.clone(),
        settings: listener_settings.clone(),
        server_activity_monitor: server_activity_monitor.clone(),
    });
    runtime.spawn(crate::network::server_connection_listener::accept_connections(listener, context));
    let started = Instant::now();
    let player_lists = match PlayerLists::load(config.server_dir.clone()) {
        Ok(player_lists) => player_lists,
        Err(throwable) => {
            report_startup_failure(&StartupFailure { throwable, saving_players: false, saved_levels: Vec::new(), snapshot: world_snapshot(&world) });
            return Ok(());
        }
    };
    info!("Preparing level \"{}\"", config.level_name);
    let created = match world {
        Some((pack_repository, mut world_stem)) => {
            world_stem.player_lists = Some(player_lists);
            Server::with_world_stem(config, pack_repository, world_stem)
        }
        None => Server::new_with_player_lists(config, Some(player_lists)),
    };
    let mut server = match created {
        Ok(server) => server,
        Err(e) => match e.get_ref().and_then(|inner| inner.downcast_ref::<StartupFailure>()) {
            Some(failure) => {
                report_startup_failure(failure);
                return Ok(());
            }
            None => return Err(e.into()),
        },
    };
    server.attach_notification_manager(notification_manager, server_activity_monitor, listener_settings);
    if let Some(management_server) = &management_server {
        management_server.set_waker(server.overworld().chunks.waker());
    }
    server.settings = settings;
    info!("Done ({}s)! For help, type \"help\"", crate::util::java_format_fixed(started.elapsed().as_nanos() as f64 / 1.0e9, 3));
    server.services = services;
    server.attach_user_lookup();
    server.task_sender = Some(task_sender);
    server.status = status;
    server.player_names_array = player_names_array;
    let mut query_thread_gs4 = None;
    let mut rcon_thread = None;
    if let Some(properties) = server.settings.as_ref().map(|s| s.properties().clone()) {
        if properties.enable_query {
            info!("Starting GS4 status listener");
            let interface = rcon::query_thread_gs4::QueryServerInterface {
                server_ip: properties.server_ip.clone(),
                server_port: port,
                server_name: server.config.motd.clone(),
                max_players: server.config.max_players,
                level_id_name: server.config.level_name.clone(),
                player_names_array: server.player_names_array.clone(),
            };
            query_thread_gs4 = rcon::query_thread_gs4::QueryThreadHandle::create(interface, properties.query_port, &runtime);
        }
        if properties.enable_rcon {
            info!("Starting remote control listener");
            rcon_thread = rcon::rcon_thread::RconThread::create(&properties.server_ip, properties.rcon_port, &properties.rcon_password, sender.clone(), &runtime);
        }
    }
    let timing = dedicated::server_watchdog::ServerTiming::new();
    timing.set_next_tick_time(Instant::now(), server.tick_rate_manager.milliseconds_per_tick());
    if let Ok(mut snapshot) = timing.snapshot.lock() {
        *snapshot = server.watchdog_snapshot();
    }
    let max_tick_time = server.settings.as_ref().map_or(0, |s| s.properties().max_tick_time);
    if max_tick_time > 0 {
        dedicated::server_watchdog::start(timing.clone(), max_tick_time, PathBuf::from("."));
    }
    server.save_everything(false, true, true);
    server.chunk_io_error_reporter.set_running();
    server.notification_manager.server_started();
    server.status_icon = server.load_status_icon();
    server.build_server_status();
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run_server_loop(&mut server, &mut events, &tasks, &timing)));
    if let Err(panic) = result {
        let (reported, message) = match panic.downcast::<crate::crash_report::ReportedException>() {
            Ok(reported) => {
                let message = format!("net.minecraft.ReportedException: {}\nCaused by: {}", reported.0.title(), reported.0.exception());
                (Some(reported.0), message)
            }
            Err(panic) => (None, crate::crash_report::exception_of(&panic)),
        };
        error!("Encountered an unexpected exception\n{message}");
        let mut report = reported.unwrap_or_else(|| crate::crash_report::CrashReport::new("Exception in server tick loop", message, crash_frames()));
        dedicated::server_watchdog::fill_system_report(&mut report, &server.watchdog_snapshot());
        let file = crate::crash_report::crash_report_file(Path::new("."));
        if report.save_to_file(&file, &crate::crash_report::ReportType::CRASH) {
            error!("This crash report has been saved to: {}", std::path::absolute(&file).unwrap_or(file).display());
        } else {
            error!("We were unable to save this crash report to disk.");
        }
    }
    timing.stop();
    if std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| server.stop_server())).is_err() {
        error!("Exception stopping the server");
    }
    if let Some(rcon_thread) = rcon_thread {
        rcon_thread.stop();
    }
    if let Some(query_thread_gs4) = query_thread_gs4 {
        query_thread_gs4.stop();
    }
    if let Some(management_server) = management_server {
        management_server.stop();
    }
    std::thread::sleep(Duration::from_millis(100));
    Ok(())
}

thread_local! {
    static PANIC_FRAMES: std::cell::RefCell<Option<Vec<String>>> = const { std::cell::RefCell::new(None) };
}

fn backtrace_frames() -> Vec<String> {
    std::backtrace::Backtrace::force_capture()
        .to_string()
        .lines()
        .filter_map(|line| line.trim().split_once(": ").map(|(_, frame)| frame.to_string()))
        .collect()
}

pub(crate) fn capture_panic_frames(location: Option<String>) {
    let frames = backtrace_frames();
    let end = frames.iter().position(|frame| frame.contains("catch_unwind")).unwrap_or(frames.len());
    let start = frames[..end].iter().rposition(|frame| frame.starts_with("std::panicking::") || frame.starts_with("core::panicking::") || frame.starts_with("core::option::")).map_or(0, |i| i + 1);
    PANIC_FRAMES.set(Some(location.into_iter().chain(frames[start..].iter().cloned()).collect()));
}

pub(crate) fn crash_frames() -> Vec<String> {
    PANIC_FRAMES.take().unwrap_or_else(backtrace_frames)
}

pub(crate) const RUN_SERVER_INITIALIZE_FRAME: &str = "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:774)";
pub(crate) const RUN_SERVER_FRAME: &str = "net.minecraft.server.MinecraftServer.runServer(MinecraftServer.java:720)";
pub(crate) const SPIN_FRAME: &str = "net.minecraft.server.MinecraftServer.lambda$spin$0(MinecraftServer.java:310)";
pub(crate) const THREAD_RUN_FRAME: &str = "java.base/java.lang.Thread.run(Thread.java:1474)";
pub(crate) const INIT_SERVER_FRAME: &str = "net.minecraft.server.dedicated.DedicatedServer.initServer(DedicatedServer.java:206)";

#[derive(Debug)]
pub struct StartupFailure {
    pub throwable: JavaThrowable,
    pub saving_players: bool,
    pub saved_levels: Vec<String>,
    pub snapshot: Option<dedicated::server_watchdog::ServerSnapshot>,
}

fn world_snapshot(world: &Option<(PackRepository, WorldStem)>) -> Option<dedicated::server_watchdog::ServerSnapshot> {
    world.as_ref().map(|(pack_repository, world_stem)| dedicated::server_watchdog::ServerSnapshot {
        world_gen_settings_lifecycle: world_stem.world_data.world_gen_settings_lifecycle,
        ..system_report_snapshot(pack_repository, world_stem.world_data.data_configuration.enabled_features, world_stem.gen_settings.options().seed(), None)
    })
}

impl std::fmt::Display for StartupFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.throwable.fmt(formatter)
    }
}

impl std::error::Error for StartupFailure {}

pub(crate) fn report_startup_failure(failure: &StartupFailure) {
    error!("Encountered an unexpected exception\n{}", failure.throwable.print_stack_trace().trim_end());
    let mut report = crate::crash_report::CrashReport::new("Exception in server tick loop", failure.throwable.description.clone(), failure.throwable.stack_trace.iter().map(ToString::to_string).collect());
    if let Some(snapshot) = &failure.snapshot {
        dedicated::server_watchdog::fill_system_report(&mut report, snapshot);
    }
    let file = crate::crash_report::crash_report_file(Path::new("."));
    if report.save_to_file(&file, &crate::crash_report::ReportType::CRASH) {
        error!("This crash report has been saved to: {}", std::path::absolute(&file).unwrap_or(file).display());
    } else {
        error!("We were unable to save this crash report to disk.");
    }
    info!("Stopping server");
    if failure.saving_players {
        info!("Saving players");
    }
    info!("Saving worlds");
    for level in &failure.saved_levels {
        info!("Saving chunks for level {level}");
    }
}

fn run_server_loop(server: &mut Server, events: &mut mpsc::UnboundedReceiver<Event>, tasks: &std::sync::mpsc::Receiver<ServerTask>, timing: &dedicated::server_watchdog::ServerTiming) {
    const OVERLOADED_THRESHOLD: Duration = Duration::from_secs(1);
    const OVERLOADED_WARNING_INTERVAL: Duration = Duration::from_secs(10);
    const SNAPSHOT_INTERVAL: u64 = 20;
    let mut next_tick = Instant::now();
    let mut last_overload_warning = next_tick;
    let mut ticks: u64 = 0;
    while !server.stopping {
        let this_tick = if server.tick_rate_manager.is_sprinting() && server.check_should_sprint_this_tick() {
            next_tick = Instant::now();
            last_overload_warning = next_tick;
            Duration::ZERO
        } else {
            let this_tick = Duration::from_nanos(server.tick_rate_manager.nanoseconds_per_tick() as u64);
            if let Some(behind) = Instant::now().checked_duration_since(next_tick)
                && behind > OVERLOADED_THRESHOLD + 20 * this_tick
                && next_tick.saturating_duration_since(last_overload_warning) >= OVERLOADED_WARNING_INTERVAL + 100 * this_tick
            {
                let ticks = (behind.as_nanos() / this_tick.as_nanos()) as u32;
                warn!("Can't keep up! Is the server overloaded? Running {}ms or {ticks} ticks behind", behind.as_millis());
                next_tick += this_tick * ticks;
                last_overload_warning = next_tick;
            }
            this_tick
        };
        let sprinting = this_tick.is_zero();
        if server.debug_command_profiler_delay_start {
            server.debug_command_profiler_delay_start = false;
            server.debug_command_profiler = Some(commands::debug_command::TimeProfiler::new(server.tick_count));
        }
        next_tick += this_tick;
        timing.set_next_tick_time(next_tick, server.tick_rate_manager.milliseconds_per_tick());
        if ticks.is_multiple_of(SNAPSHOT_INTERVAL)
            && let Ok(mut snapshot) = timing.snapshot.lock()
        {
            *snapshot = server.watchdog_snapshot();
        }
        ticks += 1;
        for level in &mut server.levels {
            level.chunks.tick_deadline = Some(next_tick);
        }
        server.refresh_spawn_protection();
        while let Ok(event) = events.try_recv() {
            server.handle(event);
        }
        for level in &mut server.levels {
            level.chunks.run_light_updates();
            level.process_chunk_events();
        }
        server.tick();
        server.run_all_tasks();
        server.mark_ready();
        let wakeup = server.overworld().chunks.main_thread_wakeup();
        loop {
            let seen = wakeup.generation();
            while let Ok(task) = tasks.try_recv() {
                task(server);
            }
            let Some(wait) = next_tick.checked_duration_since(Instant::now()).filter(|w| !w.is_zero()) else { break };
            if !server.poll_task() {
                wakeup.wait(seen, wait);
            }
        }
        if sprinting {
            server.tick_rate_manager.end_tick_work();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::level::BlockGetter;
    use crate::world::level::chunk::ChunkGenerator;

    #[test]
    fn normal_world_spawns_on_terrain_and_reloads() {
        let dir = std::env::temp_dir().join(format!("wafer-normal-world-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = ServerConfig { seed: Some(12345), world: Some(dir.clone()), ..ServerConfig::default() };
        let mut server = Server::new(config.clone()).unwrap();
        let spawn = server.spawn.pos;
        assert!(((spawn.x >> 4) - 6).abs() <= 5 && ((spawn.z >> 4) + 2).abs() <= 5, "{spawn:?}");
        assert!(wafer_core::shapes::is_face_full(server.levels[0].chunks.block_state(spawn.below()).collision_voxel_shape(), wafer_core::Direction::Up), "{spawn:?}");
        assert!(server.save_everything(true, true, true));
        drop(server);
        let server = Server::new(config).unwrap();
        assert_eq!(server.spawn.pos, spawn);
        assert!(matches!(&server.levels[0].chunks.generator, ChunkGenerator::Noise(g) if g.seed() == 12345));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn saved_data_is_written_only_when_dirty() {
        let dir = std::env::temp_dir().join(format!("wafer-dirty-saved-data-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let config = ServerConfig { world: Some(dir.clone()), ..crate::server::level::testing::flat_config() };
        let mut server = crate::server::level::testing::server_with(config);
        let file = |name: &str| dir.join("data/minecraft").join(format!("{name}.dat"));
        let names = ["world_gen_settings", "world_clocks", "weather", "game_rules"];
        let written = |server: &mut Server| {
            server.save_everything(true, true, true);
            let written: Vec<&str> = names.into_iter().filter(|name| file(name).exists()).collect();
            names.into_iter().for_each(|name| {
                let _ = std::fs::remove_file(file(name));
            });
            written
        };
        assert_eq!(written(&mut server), names);
        assert_eq!(written(&mut server), Vec::<&str>::new());
        server.game_rules.set(wafer_data::game_rule::KEEP_INVENTORY, 1);
        assert_eq!(written(&mut server), ["game_rules"]);
        server.clocks.set_total_ticks(0, 5);
        assert_eq!(written(&mut server), ["world_clocks"]);
        server.weather.set(server.weather.get());
        assert_eq!(written(&mut server), ["weather"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn hashed_seed_matches_vanilla() {
        assert_eq!(super::hashed_seed(-8153440939324367893), 0x441855f4320ff0d7);
    }
}
