use std::io;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::{Map, Value};

use crate::datafixers::data_fixer_upper::DataFixerBuilder;
use crate::datafixers::dynamic::Dynamic;
use crate::datafixers::schemas::schema::{Schema, SchemaClass};
use crate::util::basic_file_attributes::BasicFileAttributes;
use crate::util::datafix::data_fix_types::{DataFixTypes, current_version};
use crate::util::file_util;
use crate::util::java_nio::{self, java_io_error};
use crate::util::worldupdate::upgrade_progress::{Status, Type, UpgradeProgress};
use crate::world::level::storage::WorldData;
use crate::world::level::storage::level_storage_source::LevelStorageAccess;

use super::aborted_file_fix_exception::AbortedFileFixException;
use super::access::compressed_nbt::{read_compressed, write_compressed};
use super::atomic_move_not_supported_file_fix_exception::AtomicMoveNotSupportedFileFixException;
use super::failed_cleanup_file_fix_exception::FailedCleanupFileFixException;
use super::file_fix::FileFix;
use super::file_fix_exception::{FileFixException, FileFixFailure};
use super::file_system_capabilities::FileSystemCapabilities;
use super::virtualfilesystem::copy_on_write_file_system::CopyOnWriteFileSystem;
use super::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use super::virtualfilesystem::file_move::FileMove;
use super::virtualfilesystem::files;

const FILE_FIXER_INTRODUCTION_VERSION: i32 = 4772;
const FILE_FIX_DIRECTORY_NAME: &str = "filefix";
const NEW_WORLD_TEMP_NAME: &str = "new_world";
const UPGRADE_IN_PROGRESS_NAME: &str = "upgrade_in_progress.json";

pub struct FileFixerUpper {
    file_fixes: Vec<FileFix>,
    latest_file_fixer_version: i32,
}

impl FileFixerUpper {
    pub fn new(file_fixes: Vec<FileFix>, latest_file_fixer_version: i32) -> FileFixerUpper {
        FileFixerUpper { file_fixes, latest_file_fixer_version }
    }

    pub fn world_version_to_file_fixer_version(level_data_version: i32) -> i32 {
        if level_data_version < FILE_FIXER_INTRODUCTION_VERSION { 0 } else { level_data_version }
    }

    pub fn requires_file_fixing(&self, level_data_version: i32) -> bool {
        FileFixerUpper::world_version_to_file_fixer_version(level_data_version) < self.latest_file_fixer_version
    }

    pub fn fix(&self, world_access: &mut LevelStorageAccess, level_data_tag: Dynamic, upgrade_progress: &UpgradeProgress) -> Result<Dynamic, FileFixFailure> {
        self.fix_to(world_access, level_data_tag, upgrade_progress, current_version())
    }

    pub fn fix_to(
        &self,
        world_access: &mut LevelStorageAccess,
        mut level_data_tag: Dynamic,
        upgrade_progress: &UpgradeProgress,
        to_version: i32,
    ) -> Result<Dynamic, FileFixFailure> {
        let mut loaded_version = level_data_tag.get("DataVersion").as_int(-1);
        if self.requires_file_fixing(loaded_version) {
            info!("Starting upgrade for world \"{}\"", world_access.get_level_id());
            upgrade_progress.notifications().world_upgrade_started();
            let world_folder = world_access.get_level_directory().path().to_path_buf();
            let file_fix_directory = world_folder.join(FILE_FIX_DIRECTORY_NAME);
            let temp_world = file_fix_directory.join(NEW_WORLD_TEMP_NAME);
            let moves = match self.start_or_continue_file_fixing(upgrade_progress, to_version, &world_folder, &temp_world, &file_fix_directory, loaded_version) {
                Ok(moves) => moves,
                Err(FileFixFailure::Io(e)) => {
                    upgrade_progress.notifications().world_upgrade_failed("The world upgrade failed during file fixing");
                    return Err(AbortedFileFixException::of(e).into());
                }
                Err(FileFixFailure::Exception(e)) => return Err(e.into()),
            };
            if let Err(e) = swap_in_fixed_world(world_access, &moves, &file_fix_directory, &temp_world, upgrade_progress) {
                if let FileFixException::Aborted(aborted) = &e {
                    if aborted.not_reverted_moves().is_empty() {
                        cleanup(&file_fix_directory);
                        upgrade_progress.notifications().world_upgrade_failed("The world upgrade failed while swapping in the file-fixed world");
                    } else {
                        upgrade_progress
                            .notifications()
                            .world_upgrade_failed("The world upgrade failed while swapping in the file-fixed world and some moves were not reverted");
                    }
                }
                return Err(e.into());
            }
            level_data_tag = match world_access.get_unfixed_data_tag(false) {
                Ok(level_data_tag) => level_data_tag,
                Err(e) => {
                    upgrade_progress.notifications().world_upgrade_failed("The world upgrade failed while reading level.dat");
                    return Err(FileFixFailure::Io(e));
                }
            };
            loaded_version = level_data_tag.get("DataVersion").as_int(-1);
        }
        let fixed_level_data_tag = DataFixTypes::Level.update_to_current_version(level_data_tag, loaded_version);
        Ok(add_versions_to_level_data(fixed_level_data_tag, to_version))
    }

    fn start_or_continue_file_fixing(
        &self,
        upgrade_progress: &UpgradeProgress,
        to_version: i32,
        world_folder: &Path,
        temp_world: &Path,
        file_fix_directory: &Path,
        loaded_version: i32,
    ) -> Result<Vec<FileMove>, FileFixFailure> {
        let upgrade_in_progress_file = file_fix_directory.join(UPGRADE_IN_PROGRESS_NAME);
        if upgrade_in_progress_file.exists() {
            warn!("Found previously interrupted world upgrade, attempting to continue it");
            return Ok(read_moves(world_folder, temp_world, &upgrade_in_progress_file)?);
        }
        if file_fix_directory.exists() {
            delete_directory(file_fix_directory)?;
        }
        let result = (|| -> Result<Vec<FileMove>, FileFixFailure> {
            std::fs::create_dir(file_fix_directory).map_err(|e| java_nio::for_path(e, file_fix_directory))?;
            self.apply_file_fixers_on_cow(upgrade_progress, loaded_version, to_version, world_folder, file_fix_directory, temp_world)
        })();
        if result.is_err() {
            cleanup(file_fix_directory);
        }
        result
    }

    fn apply_file_fixers_on_cow(
        &self,
        upgrade_progress: &UpgradeProgress,
        loaded_version: i32,
        to_version: i32,
        world_folder: &Path,
        file_fix_directory: &Path,
        temp_world: &Path,
    ) -> Result<Vec<FileMove>, FileFixFailure> {
        let skipped_directory = file_fix_directory.to_path_buf();
        let fs = CopyOnWriteFileSystem::create(
            &world_folder.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
            world_folder,
            &file_fix_directory.join("cow"),
            move |path| path == skipped_directory,
        )?;
        let result = (|| -> Result<Vec<FileMove>, FileFixFailure> {
            self.apply_file_fixers(upgrade_progress, loaded_version, to_version, &fs.root_path())?;
            let moves = fs.collect_move_operations(temp_world);
            std::fs::create_dir(temp_world).map_err(|e| java_nio::for_path(e, temp_world))?;
            CopyOnWriteFileSystem::create_directories(&moves.directories)?;
            CopyOnWriteFileSystem::move_files(&moves.copied_files)?;
            Ok(moves.preexisting_files)
        })();
        let closed = fs.close();
        match result {
            Ok(moves) => {
                closed?;
                Ok(moves)
            }
            Err(e) => Err(e),
        }
    }

    pub fn apply_file_fixers(&self, upgrade_progress: &UpgradeProgress, loaded_version: i32, to_version: i32, base_path: &CopyOnWriteFSPath) -> Result<(), FileFixFailure> {
        let applicable_fixers = self.get_applicable_fixers(loaded_version, to_version);
        upgrade_progress.set_type(Type::Files);
        count_files(&applicable_fixers, upgrade_progress);
        upgrade_progress.set_status(Status::Upgrading);
        upgrade_progress.set_applicable_fixer_amount(applicable_fixers.len() as i32);
        for file_fix in applicable_fixers {
            upgrade_progress.increment_running_file_fixer();
            file_fix.run_fix_operations(base_path, upgrade_progress)?;
        }
        write_updated_level_data(base_path, to_version)?;
        files::delete_if_exists(&base_path.resolve_str("level.dat_old"))?;
        files::delete_if_exists(&base_path.resolve_str("session.lock"))?;
        Ok(())
    }

    fn get_applicable_fixers(&self, from_version: i32, to_version: i32) -> Vec<&FileFix> {
        let file_fixer_from_version = FileFixerUpper::world_version_to_file_fixer_version(from_version);
        self.file_fixes.iter().filter(|file_fix| file_fix.get_version() > file_fixer_from_version && file_fix.get_version() <= to_version).collect()
    }
}

fn count_files(applicable_fixers: &[&FileFix], upgrade_progress: &UpgradeProgress) {
    upgrade_progress.set_status(Status::Counting);
    let mut total_files = 0;
    for file_fix in applicable_fixers {
        total_files += file_fix.count_file_operations() as i32;
    }
    upgrade_progress.add_total_file_fix_operations(total_files);
}

fn write_updated_level_data(world_folder: &CopyOnWriteFSPath, to_version: i32) -> io::Result<()> {
    let level_dat_path = world_folder.resolve_str("level.dat");
    let unfixed_level_dat = read_compressed(&level_dat_path)?;
    let unfixed_data_tag = match &unfixed_level_dat {
        wafer_protocol::nbt::Nbt::Compound(entries) => match entries.iter().find(|(key, _)| key == "Data") {
            Some((_, data @ wafer_protocol::nbt::Nbt::Compound(_))) => data.clone(),
            _ => wafer_protocol::nbt::Nbt::Compound(Vec::new()),
        },
        _ => wafer_protocol::nbt::Nbt::Compound(Vec::new()),
    };
    let data_version = Dynamic::new(unfixed_data_tag.clone()).get("DataVersion").as_int(-1);
    let mut fixed = DataFixTypes::Level.update(Dynamic::new(unfixed_data_tag), data_version, to_version);
    fixed = add_versions_to_level_data(fixed, to_version);
    let dynamic = fixed.empty_map().set("Data", fixed);
    write_compressed(dynamic.get_value().expect("level data"), &level_dat_path)
}

fn add_versions_to_level_data(fixed: Dynamic, to_version: i32) -> Dynamic {
    let fixed = fixed.clone().set("DataVersion", fixed.create_int(to_version));
    let fixed = WorldData::write_last_played(fixed);
    WorldData::write_version_tag(fixed)
}

fn delete_directory(directory: &Path) -> io::Result<()> {
    delete_directory_contents(directory)?;
    if directory.exists() {
        std::fs::remove_dir_all(directory).map_err(|e| java_nio::for_path(e, directory))?;
    }
    Ok(())
}

fn delete_directory_contents(directory: &Path) -> io::Result<()> {
    for entry in std::fs::read_dir(directory).map_err(|e| java_nio::for_path(e, directory))? {
        let path = entry.map_err(|e| java_nio::for_path(e, directory))?.path();
        let is_directory = BasicFileAttributes::read(&path).is_ok_and(|attributes| attributes.directory);
        if is_directory {
            delete_directory_contents(&path)?;
        } else {
            let _ = java_nio::delete_if_exists(&path);
        }
    }
    let _ = java_nio::delete_if_exists(directory);
    Ok(())
}

fn cleanup(file_fix_directory: &Path) {
    if let Err(e) = delete_directory(file_fix_directory) {
        error!("Failed to clean up\n{e}");
    }
}

fn swap_in_fixed_world(
    world_access: &mut LevelStorageAccess,
    moves: &[FileMove],
    file_fix_directory: &Path,
    temp_world: &Path,
    upgrade_progress: &UpgradeProgress,
) -> Result<(), FileFixException> {
    let world_folder = world_access.get_level_directory().path().to_path_buf();
    let saves_directory = world_folder.parent().map(Path::to_path_buf).unwrap_or_default();
    let world_name = world_folder.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
    let detected = (|| -> io::Result<(FileSystemCapabilities, PathBuf, PathBuf)> {
        let file_system_capabilities = detect_file_system_capabilities(file_fix_directory)?;
        let temp_world_top_level = saves_directory.join(file_util::find_available_name(&saves_directory, &format!("{world_name} upgraded"), "")?);
        let old_world_folder = saves_directory.join(file_util::find_available_name(&saves_directory, &format!("{world_name} OUTDATED"), "")?);
        Ok((file_system_capabilities, temp_world_top_level, old_world_folder))
    })();
    let (file_system_capabilities, temp_world_top_level, old_world_folder) = detected.map_err(AbortedFileFixException::of)?;
    if !file_system_capabilities.atomic_move() {
        return Err(AtomicMoveNotSupportedFileFixException::new(file_system_capabilities));
    }
    let move_options = file_system_capabilities.get_move_options();
    info!("File system capabilities: {}", file_system_capabilities);
    let moves_file = file_fix_directory.join(UPGRADE_IN_PROGRESS_NAME);
    let prepared = if file_system_capabilities.hard_links() {
        CopyOnWriteFileSystem::hard_link_files(moves)
    } else {
        write_moves(moves, &world_folder, temp_world, &moves_file)
    };
    prepared.map_err(|e| AbortedFileFixException::new(e, Vec::new(), Some(file_system_capabilities)))?;
    info!("Applying file structure changes for world \"{}\"", world_access.get_level_id());
    if file_system_capabilities.hard_links() {
        info!("Moving new hardlinked world to top level");
        if let Err(e) = java_nio::move_file(temp_world, &temp_world_top_level, &move_options) {
            error!("Encountered error trying to move world folder:\n{e}");
            return Err(AbortedFileFixException::new(e, Vec::new(), Some(file_system_capabilities)));
        }
    } else {
        let moved = (|| -> io::Result<()> {
            info!("Moving files into new file structure");
            CopyOnWriteFileSystem::move_files_with_retry(moves, &move_options)?;
            info!("Moving new world to top level");
            java_nio::move_file(temp_world, &temp_world_top_level, &move_options)
        })();
        if let Err(e) = moved {
            error!("Encountered error while trying to create new world folder:\n{e}");
            let failed_moves = CopyOnWriteFileSystem::try_revert_moves(moves, &move_options);
            if failed_moves.is_empty() {
                if let Err(e2) = java_nio::delete_if_exists(&moves_file) {
                    warn!("Failed to delete {}\n{e2}", moves_file.display());
                }
            }
            return Err(AbortedFileFixException::new(e, failed_moves, Some(file_system_capabilities)));
        }
        info!("Complete move");
        if let Err(e) = java_nio::delete_if_exists(&moves_file) {
            warn!("Failed to delete {}\n{e}", moves_file.display());
        }
    }
    info!("Start cleanup");
    let deleted = (|| -> io::Result<()> {
        java_nio::delete_if_exists(&world_folder.join("level.dat"))?;
        java_nio::delete_if_exists(&world_folder.join("level.dat_old"))?;
        Ok(())
    })();
    if let Err(e) = deleted {
        warn!("Failed to delete outdated level.dat files: \n{e}");
    }
    let mut succeeded = false;
    let released = world_access.release_temporarily_and_run(|| -> Result<(), FileFixException> {
        info!("Moving out old world folder");
        if let Err(e) = java_nio::move_file(&world_folder, &old_world_folder, &move_options) {
            warn!("Failed to move outdated world folder out of the way; will try to delete instead: \n{e}");
            if let Err(e2) = delete_directory(&world_folder) {
                warn!("Failed to delete outdated world folder: \n{e2}");
                return Err(FailedCleanupFileFixException::new(e, file_name_of(&temp_world_top_level), file_system_capabilities));
            }
        }
        info!("Moving in new world folder");
        if let Err(e) = java_nio::move_file(&temp_world_top_level, &world_folder, &move_options) {
            warn!("Failed to move in new world folder: \n{e}");
            return Err(FailedCleanupFileFixException::new(e, file_name_of(&temp_world_top_level), file_system_capabilities));
        }
        succeeded = true;
        Ok(())
    });
    match released {
        Ok(Ok(())) => {}
        Ok(Err(e)) => return Err(e),
        Err(e) => {
            let new_world_folder = if succeeded { &world_folder } else { &temp_world_top_level };
            return Err(FailedCleanupFileFixException::new(e, file_name_of(new_world_folder), file_system_capabilities));
        }
    }
    info!("Done applying file structure changes for world \"{}\". Cleaning up outdated data...", world_access.get_level_id());
    let cleaned = (|| -> io::Result<()> {
        if old_world_folder.exists() {
            delete_directory(&old_world_folder)?;
        }
        Ok(())
    })();
    if let Err(e) = cleaned {
        warn!("Failed to clean up old world folder\n{e}");
    }
    info!("Upgrade done for world \"{}\"", world_access.get_level_id());
    upgrade_progress.notifications().world_upgrade_finished();
    Ok(())
}

fn file_name_of(path: &Path) -> String {
    path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default()
}

fn write_moves(moves: &[FileMove], from_directory: &Path, to_directory: &Path, file_path: &Path) -> io::Result<()> {
    let mut encoded = Vec::with_capacity(moves.len());
    for file_move in moves {
        encoded.push(file_move.encode(from_directory, to_directory).map_err(|e| io::Error::other(format!("java.lang.IllegalStateException: {e}")))?);
    }
    let mut object = Map::new();
    object.insert("moves".into(), Value::Array(encoded));
    let json = serde_json::to_string_pretty(&Value::Object(object)).expect("json").replace('<', "\\u003c").replace('>', "\\u003e").replace('&', "\\u0026").replace('=', "\\u003d").replace('\'', "\\u0027");
    use std::io::Write;
    let mut file = std::fs::OpenOptions::new().create(true).write(true).open(file_path).map_err(|e| java_nio::for_path(e, file_path))?;
    file.write_all(json.as_bytes())?;
    file.sync_data()
}

fn read_moves(from_directory: &Path, to_directory: &Path, file_path: &Path) -> io::Result<Vec<FileMove>> {
    let text = std::fs::read_to_string(file_path).map_err(|e| java_nio::for_path(e, file_path))?;
    let json: Value = serde_json::from_str(&text).map_err(|e| io::Error::other(format!("com.google.gson.JsonSyntaxException: {e}")))?;
    let moves = match json.get("moves") {
        Some(Value::Array(moves)) => moves,
        _ => return Err(io::Error::other("java.lang.IllegalStateException: No key moves in MapLike")),
    };
    moves
        .iter()
        .map(|file_move| FileMove::decode(file_move, from_directory, to_directory).map_err(|e| io::Error::other(format!("java.lang.IllegalStateException: {e}"))))
        .collect()
}

pub fn detect_file_system_capabilities(dir: &Path) -> io::Result<FileSystemCapabilities> {
    Ok(FileSystemCapabilities::new(supports_atomic_move(dir)?, supports_hard_links(dir)?))
}

fn random_uuid_string() -> String {
    let mut bytes: [u8; 16] = rand::random();
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    let hex: String = bytes.iter().map(|byte| format!("{byte:02x}")).collect();
    format!("{}-{}-{}-{}-{}", &hex[0..8], &hex[8..12], &hex[12..16], &hex[16..20], &hex[20..32])
}

fn supports_atomic_move(dir: &Path) -> io::Result<bool> {
    let source_file = dir.join(random_uuid_string());
    let target_file = dir.join(random_uuid_string());
    let result = (|| -> io::Result<bool> {
        std::fs::File::create_new(&source_file).map_err(|e| java_nio::for_path(e, &source_file))?;
        match std::fs::rename(&source_file, &target_file) {
            Ok(()) => Ok(true),
            Err(e) if e.kind() == io::ErrorKind::Unsupported => Ok(false),
            Err(e) => Err(java_io_error(e.kind(), "java.nio.file.FileSystemException", format!("{} -> {}: {}", source_file.display(), target_file.display(), e))),
        }
    })();
    let first = java_nio::delete_if_exists(&source_file);
    let second = java_nio::delete_if_exists(&target_file);
    let value = result?;
    first?;
    second?;
    Ok(value)
}

fn supports_hard_links(dir: &Path) -> io::Result<bool> {
    let source_file = dir.join(random_uuid_string());
    let target_file = dir.join(random_uuid_string());
    let result = (|| -> io::Result<bool> {
        std::fs::File::create_new(&source_file).map_err(|e| java_nio::for_path(e, &source_file))?;
        Ok(std::fs::hard_link(&source_file, &target_file).is_ok())
    })();
    let first = java_nio::delete_if_exists(&source_file);
    let second = java_nio::delete_if_exists(&target_file);
    let value = result?;
    first?;
    second?;
    Ok(value)
}

pub struct Builder {
    pub file_fixes: Vec<FileFix>,
    current_version: i32,
    latest_file_fixer_version: i32,
    known_schemas: Vec<Arc<Schema>>,
}

impl Builder {
    pub fn new(current_version: i32) -> Builder {
        Builder { file_fixes: Vec::new(), current_version, latest_file_fixer_version: 0, known_schemas: Vec::new() }
    }

    pub fn add_fixer(&mut self, file_fix: FileFix) {
        if !self.known_schemas.iter().any(|schema| Arc::ptr_eq(schema, file_fix.get_schema())) {
            panic!("Tried to add file fixer with unknown schema. Add it through FileFixerUpper#addSchema instead");
        }
        let file_fix_version = file_fix.get_version();
        if file_fix.get_version() > self.current_version {
            panic!("Tried to add too recent file fix for version: {}. The data version of the game is: {}", file_fix_version, self.current_version);
        }
        if let Some(last) = self.file_fixes.last() {
            if last.get_version() > file_fix_version {
                panic!("Tried to add too recent file fix for version: {}. The most recent file fix version is {}", file_fix_version, last.get_version());
            }
        }
        self.file_fixes.push(file_fix);
    }

    pub fn add_schema(&mut self, fixer_upper: &mut DataFixerBuilder, version: i32, factory: &'static SchemaClass) -> Arc<Schema> {
        self.latest_file_fixer_version = version.max(self.latest_file_fixer_version);
        let schema = fixer_upper.add_schema(version, factory);
        self.known_schemas.push(schema.clone());
        schema
    }

    pub fn build(self) -> FileFixerUpper {
        FileFixerUpper::new(self.file_fixes, self.latest_file_fixer_version)
    }
}
