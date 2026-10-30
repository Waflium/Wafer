use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use super::dedicated::dedicated_server_settings::DedicatedServerSettings;
use super::dedicated::dedicated_server_properties::{DedicatedServerProperties, InvalidProperty};
use super::eula::Eula;
use super::packs::repository::PackRepository;
use super::packs::resources::MultiPackResourceManager;
use super::reloadable_server_resources::ReloadableServerResources;
use super::world_stem::{ConstructorSavedData, WorldStem};
use super::configure_pack_repository;
use crate::tags::{ExistingTagLoading, load_tags_for_existing_registries};
use crate::world::flag::FeatureFlagSet;
use crate::world::level::data_pack_config::DataPackConfig;
use crate::world::level::world_data_configuration::WorldDataConfiguration;
use crate::util::lifecycle::Lifecycle;
use crate::server::level::game_mode::GameType;
use crate::world::difficulty::Difficulty;
use crate::world::level::dimension::level_stem::LevelStem;
use crate::world::level::levelgen::presets::world_presets::{self, NORMAL};
use crate::world::level::levelgen::world_gen_settings::WorldGenSettings;
use crate::world::level::levelgen::world_options::WorldOptions;
use wafer_protocol::nbt::Nbt;

use crate::world::level::storage::level_storage_source::{LevelDataAndDimensions, LevelStorageAccess, LevelStorageSource, get_level_data_and_dimensions};
use crate::world::level::storage::{WorldData, WorldDir};
use crate::util::datafix::data_fixers::get_file_fixer;
use crate::util::java_integer::parse_int;
use crate::util::java_throwable::JavaThrowable;
use crate::util::worldupdate::upgrade_progress::UpgradeProgress;
use super::notifications::NotificationManager;
use crate::{Listen, ServerConfig, serve_world};

const HELP: &[&str] = &[
    "Option                 Description                                         ",
    "------                 -----------                                         ",
    "--bonusChest                                                               ",
    "--demo                                                                     ",
    "--eraseCache                                                               ",
    "--forceUpgrade                                                             ",
    "--help                                                                     ",
    "--initSettings         Initializes 'server.properties' and 'eula.txt', then",
    "                         quits                                             ",
    "--nogui                                                                    ",
    "--pidFile <Path>                                                           ",
    "--port <Integer>       (default: -1)                                       ",
    "--recreateRegionFiles                                                      ",
    "--rfrProfile                                                               ",
    "--safeMode             Loads level with vanilla datapack only              ",
    "--serverId <String>                                                        ",
    "--universe <String>    (default: .)                                        ",
    "--world <String>                                                           ",
];

const FLAGS: [&str; 10] = ["nogui", "initSettings", "demo", "bonusChest", "forceUpgrade", "eraseCache", "recreateRegionFiles", "safeMode", "help", "rfrProfile"];
const WITH_ARGUMENT: [&str; 5] = ["universe", "world", "port", "serverId", "pidFile"];

#[derive(Default)]
struct OptionSet {
    flags: Vec<&'static str>,
    values: Vec<(&'static str, String)>,
    non_options: Vec<String>,
}

impl OptionSet {
    fn has(&self, name: &str) -> bool {
        self.flags.contains(&name) || self.values.iter().any(|(n, _)| *n == name)
    }

    fn value_of(&self, name: &str) -> Option<&str> {
        self.values.iter().rev().find(|(n, _)| *n == name).map(|(_, v)| v.as_str())
    }

    fn parse(args: &[String]) -> Result<OptionSet, String> {
        let mut options = OptionSet::default();
        let mut i = 0;
        while i < args.len() {
            let arg = &args[i];
            i += 1;
            if arg == "--" {
                options.non_options.extend(args[i..].iter().cloned());
                break;
            }
            let Some(body) = arg.strip_prefix("--").or_else(|| arg.strip_prefix('-').filter(|b| !b.is_empty())) else {
                options.non_options.push(arg.clone());
                continue;
            };
            let (name, inline) = match body.split_once('=') {
                Some((name, value)) => (name, Some(value.to_string())),
                None => (body, None),
            };
            let option = resolve(name)?;
            if FLAGS.contains(&option) {
                if inline.is_some() {
                    return Err(format!("joptsimple.UnrecognizedOptionException: {name} is not a recognized option"));
                }
                options.flags.push(option);
                continue;
            }
            let value = match inline {
                Some(value) => value,
                None if i < args.len() => {
                    i += 1;
                    args[i - 1].clone()
                }
                None => return Err(format!("joptsimple.OptionMissingRequiredArgumentException: Option {option} requires an argument")),
            };
            if option == "port" && parse_int(&value).is_none() {
                return Err(format!("joptsimple.OptionArgumentConversionException: Cannot parse argument '{value}' of option port"));
            }
            options.values.push((option, value));
        }
        Ok(options)
    }
}

fn resolve(name: &str) -> Result<&'static str, String> {
    let all = FLAGS.iter().chain(WITH_ARGUMENT.iter());
    if let Some(&exact) = all.clone().find(|&&o| o == name) {
        return Ok(exact);
    }
    let candidates: Vec<&'static str> = all.filter(|o| o.starts_with(name)).copied().collect();
    match candidates.as_slice() {
        [single] => Ok(single),
        [] => Err(format!("joptsimple.UnrecognizedOptionException: {name} is not a recognized option")),
        _ => Err(format!("joptsimple.UnrecognizedOptionException: {name} is not a recognized option")),
    }
}

pub fn main(args: Vec<String>) {
    std::panic::set_hook(Box::new(|info| {
        if crate::util::is_catching_exception() {
            return;
        }
        if std::thread::current().name() == Some("Server thread") {
            crate::server::capture_panic_frames(info.location().map(ToString::to_string));
            return;
        }
        let message = info.payload().downcast_ref::<String>().map(String::as_str).or_else(|| info.payload().downcast_ref::<&str>().copied()).unwrap_or("");
        error!("Caught previously unhandled exception :\n{message}");
    }));
    if let Err(throwable) = crate::util::catch_throwable(|| start(args)) {
        error!("Failed to start the minecraft server\n{}", throwable.print_stack_trace().trim_end());
    }
}

fn start(args: Vec<String>) {
    let options = match OptionSet::parse(&args) {
        Ok(options) => options,
        Err(e) => {
            error!("Failed to start the minecraft server\n{e}");
            return;
        }
    };
    if options.has("help") {
        let separator = if cfg!(windows) { "\r\n" } else { "\n" };
        eprint!("{}", HELP.iter().map(|l| format!("{l}{separator}")).collect::<String>());
        return;
    }
    if let Some(pid_file) = options.value_of("pidFile") {
        let _ = std::fs::write(pid_file, std::process::id().to_string());
    }
    {
        let _main = crate::util::java_stack::enter("net.minecraft.server.Main.main(Main.java:104)");
        crate::crash_report::preload();
    }
    if options.has("rfrProfile") {
        crate::util::profiling::rfr::rust_profiler::instance().start(crate::util::profiling::rfr::environment::Environment::Server);
    }
    let settings_file = Path::new("server.properties");
    let settings = match DedicatedServerSettings::new(settings_file) {
        Ok(settings) => settings,
        Err(InvalidProperty(e)) => {
            error!("Failed to start the minecraft server\n{e}");
            return;
        }
    };
    settings.force_save();
    crate::world::level::chunk::storage::region_file_version::RegionFileVersion::configure(&settings.properties().region_file_compression);
    let eula_file = Path::new("eula.txt");
    let eula = Eula::new(eula_file);
    if options.has("initSettings") {
        info!("Initialized '{}' and '{}'", absolute(settings_file).display(), absolute(eula_file).display());
        return;
    }
    if !eula.has_agreed_to_eula() {
        info!("You need to agree to the EULA in order to run the server. Go to eula.txt for more info.");
        return;
    }
    let universe = PathBuf::from(options.value_of("universe").unwrap_or("."));
    let next_io_thread = AtomicUsize::new(0);
    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .thread_name_fn(move || format!("Netty Server IO #{}", next_io_thread.fetch_add(1, Ordering::Relaxed)))
        .build()
        .expect("network runtime");
    let services = {
        let _context = runtime.enter();
        super::services::Services::create()
    };
    let properties = settings.properties();
    let (management_server, management_runtime) = match super::jsonrpc::create(properties) {
        Ok(Some((server, runtime))) => (Some(server), Some(runtime)),
        Ok(None) => (None, None),
        Err(e) => {
            error!("Failed to start the minecraft server\n{e}");
            runtime.shutdown_background();
            return;
        }
    };
    let level_name = options.value_of("world").map_or_else(|| properties.level_name.clone(), str::to_string);
    if let Err(e) = std::fs::create_dir_all(&universe) {
        error!("Failed to start the minecraft server\njava.io.UncheckedIOException: {}", super::dedicated::settings::java_io_exception(&e, &universe));
        runtime.shutdown_background();
        return;
    }
    let notification_manager = NotificationManager::new(management_server.clone());
    let level_storage_source = LevelStorageSource::create_default(&universe);
    let world_dir_validator = level_storage_source.world_dir_validator().clone();
    let mut access = match level_storage_source.validate_and_create_access(&level_name) {
        Ok(access) => access,
        Err(e) => {
            error!("Failed to start the minecraft server\n{e}");
            runtime.shutdown_background();
            return;
        }
    };
    let world_dir = WorldDir { root: access.get_level_directory().path().to_path_buf() };
    let level_data_tag = match fix_world_data(&mut access, notification_manager) {
        Ok(level_data_tag) => level_data_tag,
        Err(Abort) => {
            runtime.shutdown_background();
            return;
        }
    };
    let safe_mode = options.has("safeMode");
    if safe_mode {
        warn!("Safe mode active, only vanilla datapack will be loaded");
    }
    let mut pack_repository = PackRepository::create_pack_repository(world_dir.root.join("datapacks"), world_dir_validator);
    let (init_mode, initial_data_config) = load_or_create_config(properties, level_data_tag.as_ref());
    let data_configuration = configure_pack_repository(&mut pack_repository, &initial_data_config, init_mode, safe_mode);
    let resource_manager = MultiPackResourceManager::new(pack_repository.open_all_selected());
    let registries = crate::util::catch_throwable(|| crate::core::data_registries::load(&resource_manager));
    let worker_properties = properties.clone();
    let worker_data_configuration = data_configuration.clone();
    let worker_world_dir = WorldDir { root: world_dir.root.clone() };
    let bonus_chest = options.has("bonusChest");
    let demo = options.has("demo");
    let loaded = registries.and_then(|()| {
        std::thread::scope(|scope| {
            std::thread::Builder::new()
                .name("Worker-Main-1".into())
                .spawn_scoped(scope, || {
                    crate::util::catch_throwable(|| {
                        let postponed_tags = wafer_data::prepare_tags(load_tags_for_existing_registries(&resource_manager, ExistingTagLoading::Startup));
                        wafer_data::with_pending_tags(postponed_tags, || crate::core::data_registries::validate(&resource_manager))?;
                        let datapack_dimensions = crate::core::data_registries::datapack_dimensions();
                        let registries_lifecycle = crate::core::data_registries::registries_lifecycle();
                        let level_data_and_dimensions = match &level_data_tag {
                            Some(level_data_tag) => get_level_data_and_dimensions(
                                &worker_world_dir,
                                level_data_tag,
                                worker_data_configuration,
                                &datapack_dimensions,
                                registries_lifecycle,
                                &crate::resources::registry_load_task::world_data_supplier_frames(),
                            )?,
                            None => {
                                info!("No existing world data, creating new world");
                                create_new_world_data(&worker_properties, worker_data_configuration, &datapack_dimensions, registries_lifecycle, demo, bonus_chest)?
                            }
                        };
                        Ok::<_, JavaThrowable>((postponed_tags, level_data_and_dimensions))
                    })
                    .unwrap_or_else(Err)
                })
                .expect("worker thread")
                .join()
                .unwrap_or_else(|payload| Err(crate::crash_report::throwable_of(&payload)))
        })
    });
    let (postponed_tags, LevelDataAndDimensions { world_data, gen_settings, dimensions }) = match loaded {
        Ok(loaded) => loaded,
        Err(exception) => {
            warn!(
                "Failed to load datapacks, can't proceed with server load. You can either fix your datapacks or reset to vanilla with --safeMode\n{}",
                crate::resources::registry_load_task::execution_exception(exception).print_stack_trace().trim_end()
            );
            runtime.shutdown_background();
            return;
        }
    };
    let mut data_packs = match ReloadableServerResources::load_resources(&resource_manager, postponed_tags, properties.function_permission_level) {
        Ok(data_packs) => data_packs,
        Err(exception) => {
            warn!(
                "Failed to load datapacks, can't proceed with server load. You can either fix your datapacks or reset to vanilla with --safeMode\n{}",
                crate::resources::registry_load_task::execution_exception(exception).print_stack_trace().trim_end()
            );
            runtime.shutdown_background();
            return;
        }
    };
    data_packs.update_components_and_static_registry_tags();
    data_packs.finalize_recipe_loading();
    crate::world::level::levelgen::structure::templatesystem::structure_template_manager::on_resource_manager_reload(&resource_manager);
    if let Err(e) = world_data.save_level_data(&world_dir) {
        error!("Failed to save level {}\n{e}", world_dir.root.display());
    }
    let _ = std::fs::create_dir_all(world_dir.root.join("players/data"));
    let port = options.value_of("port").and_then(parse_int).filter(|&p| p >= 0).unwrap_or(properties.server_port);
    let listen = Listen::Bind { ip: properties.server_ip.clone(), port: port as u16 };
    let saved_data = ConstructorSavedData::load(Some(&world_dir));
    let mut config = ServerConfig::from_properties(properties, &universe, level_name);
    config.demo = demo;
    if properties.code_of_conduct {
        match super::dedicated::read_code_of_conducts() {
            Ok(code_of_conducts) => config.code_of_conducts = code_of_conducts.into(),
            Err(e) => {
                error!("Failed to start the minecraft server\n{e}");
                runtime.shutdown_background();
                return;
            }
        }
    }
    let world_stem = WorldStem { data_packs, world_data, gen_settings, dimensions, saved_data: Some(saved_data), player_lists: None };
    if let Err(e) = runtime.block_on(serve_world(listen, config, services, Some(settings), true, Some((pack_repository, world_stem)), management_server)) {
        error!("Encountered an unexpected exception\n{e:#}");
    }
    runtime.shutdown_background();
    if let Some(management_runtime) = management_runtime {
        management_runtime.shutdown_background();
    }
}

pub(crate) struct Abort;

pub(crate) fn fix_world_data(access: &mut LevelStorageAccess, notification_manager: NotificationManager) -> Result<Option<Nbt>, Abort> {
    if !access.has_world_data() {
        return Ok(None);
    }
    let level_data_unfixed = match access.get_unfixed_data_tag_with_fallback() {
        Ok(level_data_unfixed) => level_data_unfixed,
        Err(e) => {
            error!("Failed to load world data. World files may be corrupted. Shutting down.\n{e}");
            return Err(Abort);
        }
    };
    let summary = match access.fix_and_get_summary_from_tag(&level_data_unfixed) {
        Ok(summary) => summary,
        Err(e) => {
            error!("Failed to start the minecraft server\n{e}");
            return Err(Abort);
        }
    };
    if summary.requires_manual_conversion() {
        info!("This world must be opened in an older version (like 1.6.4) to be safely converted");
        return Err(Abort);
    }
    if !summary.is_compatible() {
        info!("This world was created by an incompatible version.");
        return Err(Abort);
    }
    match get_file_fixer().fix(access, level_data_unfixed, &UpgradeProgress::new(notification_manager)) {
        Ok(level_data_tag) => Ok(level_data_tag.into_value()),
        Err(e) => {
            error!("Failed to start the minecraft server\n{e}");
            Err(Abort)
        }
    }
}

fn load_or_create_config(properties: &DedicatedServerProperties, level_data_tag: Option<&Nbt>) -> (bool, WorldDataConfiguration) {
    match level_data_tag {
        Some(level_data_tag) => (false, WorldDataConfiguration::parse(level_data_tag)),
        None => {
            let initial_data_pack_configuration =
                DataPackConfig { enabled: properties.initial_enabled_packs.clone(), disabled: properties.initial_disabled_packs.clone() };
            (true, WorldDataConfiguration::new(initial_data_pack_configuration, FeatureFlagSet::DEFAULT_FLAGS))
        }
    }
}

fn create_new_world_data(
    properties: &DedicatedServerProperties,
    data_configuration: WorldDataConfiguration,
    datapack_dimensions: &[(String, LevelStem)],
    registries_lifecycle: Lifecycle,
    demo_mode: bool,
    bonus_chest: bool,
) -> Result<LevelDataAndDimensions, JavaThrowable> {
    let (game_type, difficulty, hardcore, level_name, data_configuration, world_options, dimensions) = if demo_mode {
        let normal = world_presets::get(NORMAL).ok_or_else(|| {
            create_new_world_data_failed(
                JavaThrowable::new(
                    "java.lang.IllegalStateException: Missing element ResourceKey[minecraft:worldgen/world_preset / minecraft:normal]",
                    vec![
                        "net.minecraft.core.HolderGetter.lambda$getOrThrow$0(HolderGetter.java:14)",
                        "java.base/java.util.Optional.orElseThrow(Optional.java:403)",
                        "net.minecraft.core.HolderGetter.getOrThrow(HolderGetter.java:14)",
                        "net.minecraft.world.level.levelgen.presets.WorldPresets.createNormalWorldDimensions(WorldPresets.java:207)",
                        "net.minecraft.server.Main.createNewWorldData(Main.java:249)",
                    ],
                ),
            )
        })?;
        (GameType::Survival, Difficulty::Normal, false, "Demo World".to_string(), WorldDataConfiguration::default_configuration(), WorldOptions::demo_options(), normal.create_world_dimensions())
    } else {
        let world_options = if bonus_chest { properties.world_options().with_bonus_chest(true) } else { properties.world_options() };
        let dimensions = properties.create_dimensions().map_err(|mut failure| {
            failure.push_frame("net.minecraft.server.Main.createNewWorldData(Main.java:264)");
            create_new_world_data_failed(failure)
        })?;
        (properties.game_mode, properties.difficulty, properties.hardcore, properties.level_name.clone(), data_configuration, world_options, dimensions)
    };
    let final_dimensions = dimensions.bake(datapack_dimensions);
    let lifecycle = final_dimensions.lifecycle().add(registries_lifecycle);
    let world_data = WorldData::new_world(game_type, difficulty, hardcore, data_configuration, level_name, final_dimensions.special_world_property, lifecycle);
    Ok(LevelDataAndDimensions { world_data, gen_settings: WorldGenSettings::new(world_options, dimensions), dimensions: final_dimensions })
}

fn create_new_world_data_failed(mut failure: JavaThrowable) -> JavaThrowable {
    failure.push_frame("net.minecraft.server.Main.lambda$main$1(Main.java:192)");
    failure.push_frames(&crate::resources::registry_load_task::world_data_loader_frames());
    failure
}

fn absolute(path: &Path) -> PathBuf {
    std::path::absolute(path).unwrap_or_else(|_| path.to_path_buf())
}
