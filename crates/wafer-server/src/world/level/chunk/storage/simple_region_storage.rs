use std::path::{Path, PathBuf};

use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use super::RegionStorage;
use super::storage_exception;
use crate::util::datafix::data_fix_types::{DataFixTypes, current_version};
use crate::util::java_throwable::JavaThrowable;

struct WriteTarget {
    storage: RegionStorage,
    folder: PathBuf,
}

pub struct SimpleRegionStorage {
    storage: RegionStorage,
    write_target: Option<WriteTarget>,
    data_fix_type: DataFixTypes,
}

impl SimpleRegionStorage {
    pub fn new(folder: &Path, sync_writes: bool, data_fix_type: DataFixTypes) -> SimpleRegionStorage {
        SimpleRegionStorage { storage: RegionStorage::new(folder.to_path_buf(), sync_writes), write_target: None, data_fix_type }
    }

    pub fn recreating(read_folder: &Path, write_folder: &Path, sync_writes: bool, data_fix_type: DataFixTypes) -> SimpleRegionStorage {
        SimpleRegionStorage {
            storage: RegionStorage::new(read_folder.to_path_buf(), sync_writes),
            write_target: Some(WriteTarget { storage: RegionStorage::new(write_folder.to_path_buf(), sync_writes), folder: write_folder.to_path_buf() }),
            data_fix_type,
        }
    }

    pub fn read(&mut self, pos: ChunkPos) -> Result<Option<Nbt>, JavaThrowable> {
        self.storage.read(pos).map_err(|e| storage_exception::on_load_worker(&e))
    }

    pub fn log_read_failure(pos: ChunkPos, failure: &JavaThrowable) {
        warn!("Failed to read chunk {pos}\n{}", failure.print_stack_trace().trim_end());
    }

    pub fn write(&mut self, pos: ChunkPos, value: &Nbt) -> Result<(), JavaThrowable> {
        let storage = match &mut self.write_target {
            Some(target) => &mut target.storage,
            None => &mut self.storage,
        };
        storage.write(pos, value).map_err(|e| {
            let failure = storage_exception::on_store_worker(&e);
            error!("Failed to store chunk {pos}\n{}", failure.print_stack_trace().trim_end());
            failure
        })
    }

    pub fn upgrade_chunk_tag(&self, chunk_tag: Nbt, default_version: i32, data_fix_context_tag: Option<&Nbt>, target_version: i32) -> Nbt {
        upgrade_chunk_tag(chunk_tag, default_version, data_fix_context_tag, target_version, self.data_fix_type)
    }

    pub fn close(&mut self) {
        if let Err(e) = self.storage.close() {
            error!("Failed to close storage\n{}", JavaThrowable::from_io_error(&e).print_stack_trace().trim_end());
        }
        if let Some(target) = &mut self.write_target {
            if let Err(e) = target.storage.close() {
                error!("Failed to close storage\n{}", JavaThrowable::from_io_error(&e).print_stack_trace().trim_end());
            }
            if target.folder.exists() {
                let _ = std::fs::remove_dir_all(&target.folder);
            }
        }
    }
}

pub fn upgrade_chunk_tag(mut chunk_tag: Nbt, default_version: i32, data_fix_context_tag: Option<&Nbt>, target_version: i32, data_fix_type: DataFixTypes) -> Nbt {
    let version = crate::nbt::get_data_version(&chunk_tag, default_version);
    if version >= target_version {
        return chunk_tag;
    }
    inject_datafixing_context(&mut chunk_tag, data_fix_context_tag);
    let mut chunk_tag = data_fix_type.update_tag(chunk_tag, version, target_version);
    remove_datafixing_context(&mut chunk_tag);
    crate::nbt::add_data_version(&mut chunk_tag, target_version);
    chunk_tag
}

pub fn upgrade_chunk_tag_to_current_version(chunk_tag: &Nbt, default_version: i32, data_fix_type: DataFixTypes) -> std::borrow::Cow<'_, Nbt> {
    if crate::nbt::get_data_version(chunk_tag, default_version) >= current_version() {
        return std::borrow::Cow::Borrowed(chunk_tag);
    }
    std::borrow::Cow::Owned(upgrade_chunk_tag(chunk_tag.clone(), default_version, None, current_version(), data_fix_type))
}

pub fn inject_datafixing_context(chunk_tag: &mut Nbt, context_tag: Option<&Nbt>) {
    if let (Nbt::Compound(entries), Some(context_tag)) = (chunk_tag, context_tag) {
        match entries.iter_mut().find(|(key, _)| key == "__context") {
            Some((_, value)) => *value = context_tag.clone(),
            None => entries.push(("__context".to_string(), context_tag.clone())),
        }
    }
}

fn remove_datafixing_context(chunk_tag: &mut Nbt) {
    if let Nbt::Compound(entries) = chunk_tag {
        entries.retain(|(key, _)| key != "__context");
    }
}
