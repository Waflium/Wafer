use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::dynamic::Dynamic;
use crate::server::dedicated::settings::java_io_exception;
use crate::util::codec::DecodeResult;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::datafix::data_fixers::get_file_fixer;
use crate::util::directory_lock::DirectoryLock;
use crate::util::java_throwable::JavaThrowable;
use crate::util::lifecycle::Lifecycle;
use crate::world::level::chunk::storage::decode_compressed_nbt;
use crate::world::level::dimension::level_stem::LevelStem;
use crate::world::level::levelgen::world_dimensions::{Complete, WorldDimensions};
use crate::world::level::levelgen::world_gen_settings::WorldGenSettings;
use crate::world::level::levelgen::world_options::WorldOptions;
use crate::world::level::world_data_configuration::WorldDataConfiguration;
use crate::world::level::validation::content_validation_message;
use crate::world::level::validation::directory_validator::DirectoryValidator;

use super::level_summary::LevelSummary;
use super::level_version::LevelVersion;
use super::{ALLOWED_SYMLINKS_CONFIG_NAME, WorldData, WorldDir, parse_validator};

#[derive(Clone)]
pub struct LevelDirectory {
    path: PathBuf,
}

impl LevelDirectory {
    pub fn new(path: PathBuf) -> LevelDirectory {
        LevelDirectory { path }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn directory_name(&self) -> String {
        self.path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
    }

    pub fn data_file(&self) -> PathBuf {
        self.path.join("level.dat")
    }

    pub fn old_data_file(&self) -> PathBuf {
        self.path.join("level.dat_old")
    }

    pub fn icon_file(&self) -> PathBuf {
        self.path.join("icon.png")
    }

    pub fn lock_file(&self) -> PathBuf {
        self.path.join("session.lock")
    }
}

pub struct LevelStorageSource {
    base_dir: PathBuf,
    world_dir_validator: Arc<DirectoryValidator>,
}

impl LevelStorageSource {
    pub fn create_default(path: &Path) -> LevelStorageSource {
        let validator = parse_validator(&path.join(ALLOWED_SYMLINKS_CONFIG_NAME));
        LevelStorageSource { base_dir: path.to_path_buf(), world_dir_validator: Arc::new(validator) }
    }

    pub fn world_dir_validator(&self) -> &Arc<DirectoryValidator> {
        &self.world_dir_validator
    }

    pub fn get_level_path(&self, level_id: &str) -> PathBuf {
        self.base_dir.join(level_id)
    }

    pub fn get_base_dir(&self) -> &Path {
        &self.base_dir
    }

    pub fn validate_and_create_access(&self, level_id: &str) -> Result<LevelStorageAccess, String> {
        let level_path = self.get_level_path(level_id);
        let validation_results = self.world_dir_validator.validate_directory(&level_path, true)?;
        if !validation_results.is_empty() {
            return Err(format!(
                "net.minecraft.world.level.validation.ContentValidationException: {}",
                content_validation_message(&level_path, &validation_results)
            ));
        }
        LevelStorageAccess::new(level_id, level_path).map_err(|e| e.to_string())
    }
}

const MAKE_LEVEL_SUMMARY_UNKNOWN_DATA_VERSION: &str = "net.minecraft.world.level.storage.LevelStorageSource.makeLevelSummary(LevelStorageSource.java:328)";
const FIX_AND_GET_SUMMARY_FROM_TAG: &str = "net.minecraft.world.level.storage.LevelStorageSource$LevelStorageAccess.fixAndGetSummaryFromTag(LevelStorageSource.java:494)";

fn make_level_summary(data_tag: &Dynamic, level_directory: &LevelDirectory, locked: bool, data_version: i32) -> Result<LevelSummary, JavaThrowable> {
    let level_version = LevelVersion::parse(data_tag);
    let level_data_version = level_version.level_data_version();
    if level_data_version != 19132 && level_data_version != 19133 {
        return Err(JavaThrowable::new(
            format!("net.minecraft.nbt.NbtFormatException: Unknown data version: {:x}", level_data_version as u32),
            vec![MAKE_LEVEL_SUMMARY_UNKNOWN_DATA_VERSION],
        ));
    }
    let requires_manual_conversion = level_data_version != 19133;
    let requires_file_fixing = get_file_fixer().requires_file_fixing(data_version);
    Ok(LevelSummary::new(level_version, level_directory.directory_name(), requires_manual_conversion, requires_file_fixing, locked))
}

pub struct LevelStorageAccess {
    lock: Option<DirectoryLock>,
    level_directory: LevelDirectory,
    level_id: String,
}

impl LevelStorageAccess {
    pub fn new(level_id: &str, path: PathBuf) -> io::Result<LevelStorageAccess> {
        let mut access = LevelStorageAccess { lock: None, level_directory: LevelDirectory::new(path), level_id: level_id.to_string() };
        access.create_lock()?;
        Ok(access)
    }

    fn create_lock(&mut self) -> io::Result<()> {
        self.lock = Some(DirectoryLock::create(self.level_directory.path())?);
        Ok(())
    }

    pub fn release_temporarily_and_run<R>(&mut self, runnable: impl FnOnce() -> R) -> io::Result<R> {
        self.close()?;
        let result = runnable();
        self.create_lock()?;
        Ok(result)
    }

    pub fn get_level_directory(&self) -> &LevelDirectory {
        &self.level_directory
    }

    pub fn get_level_id(&self) -> &str {
        &self.level_id
    }

    fn check_lock(&self) {
        if self.lock.is_none() {
            panic!("java.lang.IllegalStateException: Lock is no longer valid");
        }
    }

    pub fn fix_and_get_summary_from_tag(&self, data_tag: &Dynamic, callers: &[&'static str]) -> Result<LevelSummary, JavaThrowable> {
        self.check_lock();
        let data_version = data_tag.get("DataVersion").as_int(-1);
        let data_tag_fixed = DataFixTypes::LevelSummary.update_to_current_version(data_tag.clone(), data_version);
        make_level_summary(&data_tag_fixed, &self.level_directory, false, data_version).map_err(|mut throwable| {
            throwable.push_frame(FIX_AND_GET_SUMMARY_FROM_TAG);
            throwable.push_frames(callers);
            throwable
        })
    }

    pub fn get_unfixed_data_tag_with_fallback(&self, callers: &[&'static str]) -> Result<Dynamic, JavaThrowable> {
        let root = self.world_dir().unfixed_data_tag_with_fallback(callers)?;
        Ok(Dynamic::new(data_compound_or_empty(root)))
    }

    pub fn get_unfixed_data_tag(&self, use_fallback: bool) -> io::Result<Dynamic> {
        self.check_lock();
        let data_file = if use_fallback { self.level_directory.old_data_file() } else { self.level_directory.data_file() };
        let root = super::get_unfixed_data_tag(&data_file, &[]).map_err(io::Error::other)?;
        Ok(Dynamic::new(data_compound_or_empty(root)))
    }

    pub fn has_world_data(&self) -> bool {
        self.level_directory.data_file().exists() || self.level_directory.old_data_file().exists()
    }

    pub fn close(&mut self) -> io::Result<()> {
        self.lock = None;
        Ok(())
    }

    fn world_dir(&self) -> WorldDir {
        WorldDir { root: self.level_directory.path().to_path_buf() }
    }
}

fn data_compound_or_empty(root: Nbt) -> Nbt {
    match root {
        Nbt::Compound(entries) => match entries.into_iter().find(|(key, _)| key == "Data") {
            Some((_, data @ Nbt::Compound(_))) => data,
            _ => Nbt::Compound(Vec::new()),
        },
        _ => Nbt::Compound(Vec::new()),
    }
}

const READ_EXISTING_SAVED_DATA_FRAME: &str = "net.minecraft.world.level.storage.LevelStorageSource.readExistingSavedData(LevelStorageSource.java:169)";
const GET_LEVEL_DATA_AND_DIMENSIONS_FRAME: &str = "net.minecraft.world.level.storage.LevelStorageSource.getLevelDataAndDimensions(LevelStorageSource.java:149)";
const DIMENSIONS_FALLBACK_FRAME: &str = "net.minecraft.world.level.storage.LevelStorageSource.lambda$getLevelDataAndDimensions$0(LevelStorageSource.java:151)";
const MAP_OR_ELSE_FRAME: &str = "com.mojang.serialization.DataResult$Error.mapOrElse(DataResult.java:309)";

pub struct LevelDataAndDimensions {
    pub world_data: WorldData,
    pub gen_settings: WorldGenSettings,
    pub dimensions: Complete,
}

fn message_of(description: &str) -> String {
    match description.split_once(": ") {
        Some((_, message)) => message.to_string(),
        None => "null".to_string(),
    }
}

pub fn read_existing_saved_data(world_dir: &WorldDir, caller_frames: &[&'static str]) -> Result<Result<WorldGenSettings, String>, JavaThrowable> {
    let data_location = world_dir.data("world_gen_settings");
    let bytes = match std::fs::read(&data_location) {
        Ok(bytes) => bytes,
        Err(e) => return Ok(Err(message_of(&java_io_exception(&e, &data_location)))),
    };
    let file_contents = match decode_compressed_nbt(&bytes) {
        Ok(file_contents) => file_contents,
        Err(description) => return Ok(Err(message_of(&description))),
    };
    let data = match file_contents.get("data") {
        Some(data @ Nbt::Compound(_)) => data.clone(),
        _ => Nbt::Compound(Vec::new()),
    };
    let mut frames = vec![READ_EXISTING_SAVED_DATA_FRAME, GET_LEVEL_DATA_AND_DIMENSIONS_FRAME];
    frames.extend_from_slice(caller_frames);
    match crate::util::codec::with_nbt_ops(|| WorldGenSettings::decode(&data, &frames))? {
        DecodeResult { value: Some(settings), error: None } => Ok(Ok(settings)),
        DecodeResult { error, .. } => Ok(Err(error.unwrap_or_default())),
    }
}

pub fn get_level_data_and_dimensions(
    world_dir: &WorldDir,
    level_data_tag: &Nbt,
    data_configuration: WorldDataConfiguration,
    datapack_dimensions: &[(String, LevelStem)],
    registries_lifecycle: Lifecycle,
    caller_frames: &[&'static str],
) -> Result<LevelDataAndDimensions, JavaThrowable> {
    let world_gen_settings = match read_existing_saved_data(world_dir, caller_frames)? {
        Ok(world_gen_settings) => world_gen_settings,
        Err(message) => {
            error!("Unable to read or access the world gen settings file! Falling back to the default settings with a random world seed. {message}");
            let mut frames = vec![DIMENSIONS_FALLBACK_FRAME, MAP_OR_ELSE_FRAME, GET_LEVEL_DATA_AND_DIMENSIONS_FRAME];
            frames.extend_from_slice(caller_frames);
            WorldGenSettings::new(WorldOptions::default_with_random_seed(), WorldDimensions::from_registry(datapack_dimensions, &frames)?)
        }
    };
    let dimensions = world_gen_settings.dimensions().bake(datapack_dimensions);
    let lifecycle = dimensions.lifecycle().add(registries_lifecycle);
    let world_data = WorldData::from_tag(level_data_tag, data_configuration, dimensions.special_world_property, lifecycle);
    Ok(LevelDataAndDimensions { world_data, gen_settings: world_gen_settings, dimensions })
}
