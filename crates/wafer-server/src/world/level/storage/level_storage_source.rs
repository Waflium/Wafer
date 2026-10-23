use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use wafer_protocol::nbt::Nbt;

use crate::datafixers::dynamic::Dynamic;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::datafix::data_fixers::get_file_fixer;
use crate::util::directory_lock::DirectoryLock;
use crate::world::level::validation::content_validation_message;
use crate::world::level::validation::directory_validator::DirectoryValidator;

use super::level_summary::LevelSummary;
use super::level_version::LevelVersion;
use super::{ALLOWED_SYMLINKS_CONFIG_NAME, WorldDir, parse_validator};

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

fn make_level_summary(data_tag: &Dynamic, level_directory: &LevelDirectory, locked: bool, data_version: i32) -> Result<LevelSummary, String> {
    let level_version = LevelVersion::parse(data_tag);
    let level_data_version = level_version.level_data_version();
    if level_data_version != 19132 && level_data_version != 19133 {
        return Err(format!("net.minecraft.nbt.NbtFormatException: Unknown data version: {:x}", level_data_version as u32));
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

    pub fn fix_and_get_summary_from_tag(&self, data_tag: &Dynamic) -> Result<LevelSummary, String> {
        self.check_lock();
        let data_version = data_tag.get("DataVersion").as_int(-1);
        let data_tag_fixed = DataFixTypes::LevelSummary.update_to_current_version(data_tag.clone(), data_version);
        make_level_summary(&data_tag_fixed, &self.level_directory, false, data_version)
    }

    pub fn get_unfixed_data_tag_with_fallback(&self) -> io::Result<Dynamic> {
        let root = self.world_dir().unfixed_data_tag_with_fallback().map_err(io::Error::other)?;
        Ok(Dynamic::new(data_compound_or_empty(root)))
    }

    pub fn get_unfixed_data_tag(&self, use_fallback: bool) -> io::Result<Dynamic> {
        self.check_lock();
        let data_file = if use_fallback { self.level_directory.old_data_file() } else { self.level_directory.data_file() };
        let root = super::read_level_data_tag_raw(&data_file).map_err(io::Error::other)?;
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
