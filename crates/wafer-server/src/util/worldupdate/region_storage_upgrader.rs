use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::Receiver;

use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use super::upgrade_progress::{Status, UpgradeProgress};
use crate::server::level::chunk_workers::{execute, worker_main_count};
use crate::util::datafix::data_fix_types::{DataFixTypes, current_version};
use crate::world::level::chunk::storage::simple_region_storage::upgrade_chunk_tag;
use crate::util::java_integer::{number_format_exception, parse_int};
use crate::util::java_throwable::JavaThrowable;
use crate::world::level::chunk::storage::RegionFile;
use crate::world::level::chunk::storage::simple_region_storage::SimpleRegionStorage;
use crate::world::level::chunk::storage::storage_exception;
use crate::world::level::storage::WorldDir;

const NEW_DIRECTORY_PREFIX: &str = "new_";

pub type TagModifier = fn(ChunkPos, &mut Nbt) -> bool;

pub struct FileToUpgrade {
    file: PathBuf,
    chunks_to_upgrade: Vec<ChunkPos>,
}

impl FileToUpgrade {
    pub fn file(&self) -> &Path {
        &self.file
    }

    pub fn chunks_to_upgrade(&self) -> &[ChunkPos] {
        &self.chunks_to_upgrade
    }
}

struct PendingPosition {
    pos: ChunkPos,
    read: Result<Option<(i32, Receiver<Result<Nbt, JavaThrowable>>)>, JavaThrowable>,
}

pub struct RegionStorageUpgrader {
    upgrade_progress: Arc<UpgradeProgress>,
    folder_name: String,
    previous_write_future: Option<Result<(), JavaThrowable>>,
    data_fix_type: DataFixTypes,
    default_version: i32,
    recreate_region_files: bool,
    dimension_key: Option<String>,
    storage: Option<SimpleRegionStorage>,
    files: Option<Vec<FileToUpgrade>>,
    start_index: i32,
    data_fix_context_tag: Option<Nbt>,
    tag_modifiers: BTreeMap<i32, TagModifier>,
}

impl RegionStorageUpgrader {
    fn new(
        data_fix_type: DataFixTypes,
        folder_name: String,
        default_version: i32,
        recreate_region_files: bool,
        upgrade_progress: Arc<UpgradeProgress>,
        start_index: i32,
        data_fix_context_tag: Option<Nbt>,
        tag_modifiers: BTreeMap<i32, TagModifier>,
    ) -> RegionStorageUpgrader {
        RegionStorageUpgrader {
            upgrade_progress,
            folder_name,
            previous_write_future: None,
            data_fix_type,
            default_version,
            recreate_region_files,
            dimension_key: None,
            storage: None,
            files: None,
            start_index,
            data_fix_context_tag,
            tag_modifiers,
        }
    }

    pub fn init(&mut self, dimension_key: &str, level_directory: &Path) {
        let region_folder = WorldDir { root: level_directory.to_path_buf() }.dimension_path(dimension_key).join(&self.folder_name);
        self.dimension_key = Some(dimension_key.to_string());
        self.storage = Some(self.create_storage(&region_folder));
        self.files = Some(self.get_files_to_process(&region_folder));
    }

    pub fn upgrade(&mut self) {
        let (Some(dimension_key), Some(_), Some(files)) = (self.dimension_key.clone(), self.storage.as_ref(), self.files.take()) else {
            panic!("java.lang.IllegalStateException: RegionStorageUpgrader has not been initialized");
        };
        if !files.is_empty() {
            let total_size = self.upgrade_progress.get_total_file_fix_stats().total_operations() as f32;
            self.upgrade_progress.set_status(Status::Upgrading);
            let mut next_index = 0usize;
            while !self.upgrade_progress.is_canceled() {
                let mut worked = false;
                let mut total_progress = 0.0f32;
                if let Some(file_to_upgrade) = files.get(next_index) {
                    next_index += 1;
                    let converted = self.process_positions(file_to_upgrade.chunks_to_upgrade());
                    worked = !file_to_upgrade.chunks_to_upgrade().is_empty();
                    if self.recreate_region_files {
                        if converted {
                            self.on_file_finished(file_to_upgrade.file());
                        } else {
                            error!("Failed to convert region file {}", storage_exception::java_path(file_to_upgrade.file()));
                        }
                    }
                }
                let current_dimension_progress = next_index as f32 / total_size;
                let current_total_progress = (self.start_index + next_index as i32) as f32 / total_size;
                self.upgrade_progress.set_dimension_progress(&dimension_key, current_dimension_progress);
                total_progress += current_total_progress;
                self.upgrade_progress.set_total_progress(total_progress);
                if !worked {
                    break;
                }
            }
            self.upgrade_progress.set_status(Status::Finished);
            if let Some(storage) = self.storage.as_mut() {
                storage.close();
            }
        }
        self.files = Some(files);
    }

    fn create_storage(&self, region_folder: &Path) -> SimpleRegionStorage {
        if self.recreate_region_files {
            SimpleRegionStorage::recreating(region_folder, &resolve_recreate_directory(region_folder), true, self.data_fix_type)
        } else {
            SimpleRegionStorage::new(region_folder, true, self.data_fix_type)
        }
    }

    fn get_files_to_process(&self, region_folder: &Path) -> Vec<FileToUpgrade> {
        let files_to_upgrade = get_all_chunk_positions(region_folder);
        self.upgrade_progress.add_total_file_fix_operations(files_to_upgrade.len() as i32);
        self.upgrade_progress.add_total_chunks(files_to_upgrade.iter().map(|file_to_upgrade| file_to_upgrade.chunks_to_upgrade().len() as i32).sum());
        files_to_upgrade
    }

    pub fn file_amount(&self) -> i32 {
        self.files.as_ref().map_or(0, |files| files.len() as i32)
    }

    fn process_positions(&mut self, positions: &[ChunkPos]) -> bool {
        let window = worker_main_count() * 2;
        let mut pending = VecDeque::new();
        let mut next = 0;
        let mut converted = true;
        while next < positions.len() || !pending.is_empty() {
            while pending.len() < window && next < positions.len() {
                pending.push_back(self.start_position(positions[next]));
                next += 1;
            }
            let Some(position) = pending.pop_front() else { break };
            converted = converted && self.process_one_position(position);
            if !converted {
                break;
            }
        }
        converted
    }

    fn start_position(&mut self, pos: ChunkPos) -> PendingPosition {
        let read = match self.storage.as_mut().expect("initialized storage").read(pos) {
            Ok(None) => Ok(None),
            Ok(Some(chunk_tag)) => {
                let version = crate::nbt::get_data_version(&chunk_tag, -1);
                let target_version = self.tag_modifiers.keys().next().copied().unwrap_or_else(current_version);
                let (default_version, data_fix_context_tag, data_fix_type) = (self.default_version, self.data_fix_context_tag.clone(), self.data_fix_type);
                let (sender, receiver) = std::sync::mpsc::channel();
                execute(move || {
                    let upgraded = crate::util::catch_throwable(|| upgrade_chunk_tag(chunk_tag, default_version, data_fix_context_tag.as_ref(), target_version, data_fix_type));
                    let _ = sender.send(upgraded);
                });
                Ok(Some((version, receiver)))
            }
            Err(failure) => Err(failure),
        };
        PendingPosition { pos, read }
    }

    fn process_one_position(&mut self, position: PendingPosition) -> bool {
        let pos = position.pos;
        let mut converted = false;
        match self.try_process_one_position(position) {
            Ok(result) => converted = result,
            Err(cause) => {
                if !cause.is_io_exception() {
                    panic!("{cause}");
                }
                error!("Error upgrading chunk {pos}\n{}", cause.print_stack_trace().trim_end());
            }
        }
        if converted {
            self.upgrade_progress.increment_converted();
        } else {
            self.upgrade_progress.increment_skipped();
        }
        converted
    }

    fn try_process_one_position(&mut self, position: PendingPosition) -> Result<bool, JavaThrowable> {
        let pos = position.pos;
        let read = position.read.inspect_err(|failure| SimpleRegionStorage::log_read_failure(pos, failure))?;
        let Some((version, receiver)) = read else { return Ok(false) };
        let mut chunk_tag = match receiver.recv() {
            Ok(Ok(chunk_tag)) => chunk_tag,
            Ok(Err(failure)) => panic!("{failure}"),
            Err(_) => panic!("java.util.concurrent.CancellationException"),
        };
        let latest_version = current_version();
        let mut changed = false;
        let tag_modifiers: Vec<(i32, TagModifier)> = self.tag_modifiers.iter().map(|(&version, &modifier)| (version, modifier)).collect();
        for (needed_version, tag_modifier) in tag_modifiers {
            chunk_tag = self.upgrade_tag(chunk_tag, needed_version);
            changed |= tag_modifier(pos, &mut chunk_tag);
        }
        let upgraded_tag = self.upgrade_tag(chunk_tag, latest_version);
        changed |= version < latest_version;
        if changed || self.recreate_region_files {
            if let Some(Err(failure)) = &self.previous_write_future {
                return Err(failure.clone());
            }
            self.previous_write_future = Some(self.storage.as_mut().expect("initialized storage").write(pos, &upgraded_tag));
            return Ok(true);
        }
        Ok(false)
    }

    fn upgrade_tag(&self, chunk_tag: Nbt, target_version: i32) -> Nbt {
        self.storage.as_ref().expect("initialized storage").upgrade_chunk_tag(chunk_tag, self.default_version, self.data_fix_context_tag.as_ref(), target_version)
    }

    fn on_file_finished(&mut self, region_file: &Path) {
        if self.recreate_region_files {
            if let Some(Err(failure)) = &self.previous_write_future {
                panic!("{failure}");
            }
            let file_path = region_file;
            let directory_path = file_path.parent().expect("a region file directory");
            let new_file_path = resolve_recreate_directory(directory_path).join(file_path.file_name().expect("a region file name"));
            if new_file_path.exists() {
                let moved = std::fs::remove_file(file_path).and_then(|()| std::fs::rename(&new_file_path, file_path));
                if let Err(e) = moved {
                    error!("Failed to replace an old region file\n{}", JavaThrowable::from_io_error(&e).print_stack_trace().trim_end());
                }
            } else {
                error!("Failed to replace an old region file. New file {} does not exist.", storage_exception::java_path(&new_file_path));
            }
        }
    }
}

fn resolve_recreate_directory(directory_path: &Path) -> PathBuf {
    let name = directory_path.file_name().expect("a directory name").to_string_lossy();
    directory_path.with_file_name(format!("{NEW_DIRECTORY_PREFIX}{name}"))
}

fn get_all_chunk_positions(region_folder: &Path) -> Vec<FileToUpgrade> {
    let Ok(entries) = std::fs::read_dir(region_folder) else { return Vec::new() };
    let mut region_file_chunks = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.ends_with(".mca") {
            continue;
        }
        let Some((x_offset, z_offset)) = region_coordinates(&name) else { continue };
        let region_file = region_folder.join(&name);
        let mut chunk_positions = Vec::new();
        match RegionFile::new(&region_file, true) {
            Ok(mut region_source) => {
                for x in 0..32 {
                    for z in 0..32 {
                        let pos = ChunkPos::new(x + x_offset, z + z_offset);
                        if region_source.does_chunk_exist(pos) {
                            chunk_positions.push(pos);
                        }
                    }
                }
                let closed = region_source.close();
                if !chunk_positions.is_empty() {
                    region_file_chunks.push(FileToUpgrade { file: region_file.clone(), chunks_to_upgrade: chunk_positions });
                }
                if let Err(e) = closed {
                    error!("Failed to read chunks from region file {}\n{}", storage_exception::java_path(&region_file), JavaThrowable::from_io_error(&e).print_stack_trace().trim_end());
                }
            }
            Err(e) => {
                error!("Failed to read chunks from region file {}\n{}", storage_exception::java_path(&region_file), JavaThrowable::from_io_error(&e).print_stack_trace().trim_end());
            }
        }
    }
    region_file_chunks
}

fn region_coordinates(name: &str) -> Option<(i32, i32)> {
    let body = name.strip_prefix("r.")?.strip_suffix(".mca")?;
    let (x, z) = body.split_once('.')?;
    let digits = |text: &str| {
        let unsigned = text.strip_prefix('-').unwrap_or(text);
        !unsigned.is_empty() && unsigned.bytes().all(|b| b.is_ascii_digit())
    };
    if !digits(x) || !digits(z) {
        return None;
    }
    let parse = |text: &str| parse_int(text).unwrap_or_else(|| panic!("java.lang.NumberFormatException: {}", number_format_exception(text)));
    Some((parse(x) << 5, parse(z) << 5))
}

#[derive(Clone)]
pub struct Builder {
    data_fix_type: Option<DataFixTypes>,
    type_: Option<String>,
    folder_name: Option<String>,
    default_version: i32,
    recreate_region_files: bool,
    upgrade_progress: Option<Arc<UpgradeProgress>>,
    data_fix_context_tag: Option<Nbt>,
    tag_modifiers: BTreeMap<i32, TagModifier>,
}

impl Builder {
    pub fn new() -> Builder {
        Builder {
            data_fix_type: None,
            type_: None,
            folder_name: None,
            default_version: -1,
            recreate_region_files: false,
            upgrade_progress: None,
            data_fix_context_tag: None,
            tag_modifiers: BTreeMap::new(),
        }
    }

    pub fn set_data_fix_type(mut self, data_fix_type: DataFixTypes) -> Builder {
        self.data_fix_type = Some(data_fix_type);
        self
    }

    pub fn set_type_and_folder_name(self, name: &str) -> Builder {
        self.set_type(name).set_folder_name(name)
    }

    pub fn set_type(mut self, type_: &str) -> Builder {
        self.type_ = Some(type_.to_string());
        self
    }

    pub fn set_folder_name(mut self, folder_name: &str) -> Builder {
        self.folder_name = Some(folder_name.to_string());
        self
    }

    pub fn set_default_version(mut self, default_version: i32) -> Builder {
        self.default_version = default_version;
        self
    }

    pub fn set_recreate_region_files(mut self, recreate_region_files: bool) -> Builder {
        self.recreate_region_files = recreate_region_files;
        self
    }

    pub fn track_progress(mut self, upgrade_progress: Arc<UpgradeProgress>) -> Builder {
        self.upgrade_progress = Some(upgrade_progress);
        self
    }

    pub fn set_data_fix_context_tag(mut self, data_fix_context_tag: Option<Nbt>) -> Builder {
        self.data_fix_context_tag = data_fix_context_tag;
        self
    }

    pub fn add_tag_modifier(mut self, version: i32, tag_modifier: TagModifier) -> Builder {
        if self.tag_modifiers.contains_key(&version) {
            panic!("java.lang.IllegalStateException: Can't add two fixers for the same data version");
        }
        self.tag_modifiers.insert(version, tag_modifier);
        self
    }

    pub fn copy(&self) -> Builder {
        self.clone()
    }

    pub fn build(self, previous_copies_file_amounts: i32) -> RegionStorageUpgrader {
        self.type_.expect("a type");
        RegionStorageUpgrader::new(
            self.data_fix_type.expect("a data fix type"),
            self.folder_name.expect("a folder name"),
            self.default_version,
            self.recreate_region_files,
            self.upgrade_progress.expect("an upgrade progress"),
            previous_copies_file_amounts,
            self.data_fix_context_tag,
            self.tag_modifiers,
        )
    }
}

impl Default for Builder {
    fn default() -> Builder {
        Builder::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::server::notifications::NotificationManager;
    use crate::world::level::chunk::storage::RegionStorage;

    fn entity_chunk(version: i32, pos: ChunkPos) -> Nbt {
        Nbt::Compound(
            vec![
                ("DataVersion".to_string(), Nbt::Int(version)),
                ("Position".to_string(), Nbt::IntArray(vec![pos.x, pos.z])),
                ("Entities".to_string(), Nbt::List(Vec::new())),
            ]
            .into(),
        )
    }

    fn scratch_directory(name: &str) -> PathBuf {
        let directory = std::env::temp_dir().join(format!("wafer-{name}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&directory);
        directory
    }

    fn upgrader(progress: &Arc<UpgradeProgress>, level_directory: &Path, recreate: bool) -> RegionStorageUpgrader {
        let mut upgrader = Builder::new()
            .set_type_and_folder_name("entities")
            .set_data_fix_type(DataFixTypes::EntityChunk)
            .set_recreate_region_files(recreate)
            .track_progress(progress.clone())
            .build(0);
        upgrader.init("minecraft:overworld", level_directory);
        upgrader
    }

    fn data_version(storage: &mut RegionStorage, pos: ChunkPos) -> i32 {
        let tag = storage.read(pos).unwrap().expect("a stored chunk");
        crate::nbt::get_data_version(&tag, -1)
    }

    #[test]
    fn old_chunks_are_upgraded_and_counted() {
        let level_directory = scratch_directory("upgrader-old");
        let folder = WorldDir { root: level_directory.clone() }.dimension_path("minecraft:overworld").join("entities");
        let mut storage = RegionStorage::new(folder.clone(), false);
        storage.write(ChunkPos::new(0, 0), &entity_chunk(2700, ChunkPos::new(0, 0))).unwrap();
        storage.write(ChunkPos::new(1, 0), &entity_chunk(2700, ChunkPos::new(1, 0))).unwrap();
        storage.close().unwrap();
        let progress = Arc::new(UpgradeProgress::new(NotificationManager::default()));
        let mut upgrader = upgrader(&progress, &level_directory, false);
        assert_eq!(upgrader.file_amount(), 1);
        assert_eq!(progress.get_total_chunks(), 2);
        upgrader.upgrade();
        assert_eq!((progress.get_converted(), progress.get_skipped()), (2, 0));
        assert_eq!(progress.get_status(), Status::Finished);
        let mut storage = RegionStorage::new(folder, false);
        assert_eq!(data_version(&mut storage, ChunkPos::new(0, 0)), current_version());
        assert_eq!(data_version(&mut storage, ChunkPos::new(1, 0)), current_version());
        std::fs::remove_dir_all(level_directory).unwrap();
    }

    #[test]
    fn an_unchanged_chunk_skips_the_rest_of_the_region_file() {
        let level_directory = scratch_directory("upgrader-skip");
        let folder = WorldDir { root: level_directory.clone() }.dimension_path("minecraft:overworld").join("entities");
        let mut storage = RegionStorage::new(folder.clone(), false);
        storage.write(ChunkPos::new(0, 0), &entity_chunk(current_version(), ChunkPos::new(0, 0))).unwrap();
        storage.write(ChunkPos::new(1, 0), &entity_chunk(2700, ChunkPos::new(1, 0))).unwrap();
        storage.close().unwrap();
        let progress = Arc::new(UpgradeProgress::new(NotificationManager::default()));
        let mut upgrader = upgrader(&progress, &level_directory, false);
        upgrader.upgrade();
        assert_eq!((progress.get_converted(), progress.get_skipped()), (0, 1));
        let mut storage = RegionStorage::new(folder, false);
        assert_eq!(data_version(&mut storage, ChunkPos::new(1, 0)), 2700);
        std::fs::remove_dir_all(level_directory).unwrap();
    }

    #[test]
    fn recreated_region_files_replace_the_old_ones() {
        let level_directory = scratch_directory("upgrader-recreate");
        let folder = WorldDir { root: level_directory.clone() }.dimension_path("minecraft:overworld").join("entities");
        let mut storage = RegionStorage::new(folder.clone(), false);
        storage.write(ChunkPos::new(0, 0), &entity_chunk(current_version(), ChunkPos::new(0, 0))).unwrap();
        storage.close().unwrap();
        let progress = Arc::new(UpgradeProgress::new(NotificationManager::default()));
        let mut upgrader = upgrader(&progress, &level_directory, true);
        upgrader.upgrade();
        assert_eq!((progress.get_converted(), progress.get_skipped()), (1, 0));
        assert!(folder.join("r.0.0.mca").is_file());
        assert!(!resolve_recreate_directory(&folder).exists());
        std::fs::remove_dir_all(level_directory).unwrap();
    }

    #[test]
    fn region_coordinates_follow_the_region_file_pattern() {
        assert_eq!(region_coordinates("r.-1.2.mca"), Some((-32, 64)));
        assert_eq!(region_coordinates("r.0.0.mca"), Some((0, 0)));
        assert_eq!(region_coordinates("r.0.mca"), None);
        assert_eq!(region_coordinates("r.a.0.mca"), None);
        assert_eq!(region_coordinates("r.0.0.mcc"), None);
    }

    #[test]
    fn tag_modifiers_run_in_version_order() {
        let builder = Builder::new().add_tag_modifier(5, |_, _| true).add_tag_modifier(2, |_, _| false);
        assert_eq!(builder.tag_modifiers.keys().copied().collect::<Vec<_>>(), [2, 5]);
    }

    #[test]
    #[should_panic(expected = "Can't add two fixers for the same data version")]
    fn two_tag_modifiers_cannot_share_a_version() {
        let _ = Builder::new().add_tag_modifier(2, |_, _| true).add_tag_modifier(2, |_, _| false);
    }
}
