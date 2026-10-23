use std::collections::HashMap;
use std::io::{self, Read, Write};
use std::path::PathBuf;
use std::sync::Mutex;

use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;

use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_path::CopyOnWriteFSPath;
use crate::util::filefix::virtualfilesystem::copy_on_write_fs_provider::OpenOptions;
use crate::util::filefix::virtualfilesystem::files;
use crate::world::level::chunk::storage::region_storage_info::RegionStorageInfo;
use crate::world::level::chunk::storage::simple_region_storage::upgrade_chunk_tag;
use crate::world::level::chunk::storage::{ExternalChunkFiles, RegionFile};

use super::AutoCloseable;

struct CowExternalChunkFiles {
    folder: CopyOnWriteFSPath,
}

impl CowExternalChunkFiles {
    fn external_chunk_path(&self, pos: ChunkPos) -> CopyOnWriteFSPath {
        self.folder.resolve_str(&format!("c.{}.{}.mcc", pos.x, pos.z))
    }
}

impl ExternalChunkFiles for CowExternalChunkFiles {
    fn read(&self, pos: ChunkPos) -> io::Result<Vec<u8>> {
        let external_file = self.external_chunk_path(pos);
        if !files::is_regular_file(&external_file) {
            error!("External chunk path {external_file} is not file");
            return Err(io::Error::from(io::ErrorKind::NotFound));
        }
        let mut data = Vec::new();
        files::new_input_stream(&external_file)?.read_to_end(&mut data)?;
        Ok(data)
    }

    fn write(&self, pos: ChunkPos, data: &[u8]) -> io::Result<()> {
        let options = OpenOptions { write: true, create: true, truncate_existing: true, ..OpenOptions::default() };
        files::new_output_stream(&self.external_chunk_path(pos), &options)?.write_all(data)
    }

    fn remove(&self, pos: ChunkPos) -> io::Result<()> {
        files::delete_if_exists(&self.external_chunk_path(pos)).map(|_| ())
    }
}

struct CowRegionFileStorage {
    folder: CopyOnWriteFSPath,
    region_cache: HashMap<(i32, i32), Option<RegionFile>>,
}

impl CowRegionFileStorage {
    fn region_path(&self, pos: ChunkPos) -> CopyOnWriteFSPath {
        self.folder.resolve_str(&format!("r.{}.{}.mca", pos.x >> 5, pos.z >> 5))
    }

    fn get_region_file(&mut self, pos: ChunkPos, create: bool) -> io::Result<Option<&mut RegionFile>> {
        let key = (pos.x >> 5, pos.z >> 5);
        let cached_is_present = match self.region_cache.get(&key) {
            Some(Some(_)) => true,
            Some(None) => {
                if !create {
                    return Ok(None);
                }
                false
            }
            None => false,
        };
        if !cached_is_present {
            let path = self.region_path(pos);
            if !create && !files::is_regular_file(&path) {
                self.region_cache.insert(key, None);
                return Ok(None);
            }
            files::create_directories_safe(&self.folder)?;
            let options = OpenOptions { create: true, read: true, write: true, ..OpenOptions::default() };
            let file = path.get_file_system().provider().new_byte_channel(&path, &options)?;
            let external = CowExternalChunkFiles { folder: self.folder.clone() };
            let new_region = RegionFile::from_file(&PathBuf::from(path.to_string()), file, Some(Box::new(external)))?;
            self.region_cache.insert(key, Some(new_region));
        }
        Ok(self.region_cache.get_mut(&key).and_then(Option::as_mut))
    }

    fn read(&mut self, pos: ChunkPos) -> io::Result<Option<Nbt>> {
        match self.get_region_file(pos, false)? {
            Some(region) => region.read(pos),
            None => Ok(None),
        }
    }

    fn write(&mut self, pos: ChunkPos, value: &Nbt) -> io::Result<()> {
        let region = self.get_region_file(pos, true)?.expect("created region file");
        region.write(pos, value)
    }

    fn close(&mut self) -> io::Result<()> {
        let mut result = Ok(());
        for region in self.region_cache.values_mut().flatten() {
            if let Err(e) = region.close() {
                result = Err(e);
            }
        }
        result
    }
}

pub struct ChunkNbt {
    info: RegionStorageInfo,
    storage: Mutex<CowRegionFileStorage>,
    data_fix_type: DataFixTypes,
    target_version: i32,
}

impl ChunkNbt {
    pub fn new(info: RegionStorageInfo, path: CopyOnWriteFSPath, type_: DataFixTypes, target_version: i32) -> ChunkNbt {
        ChunkNbt { info, storage: Mutex::new(CowRegionFileStorage { folder: path, region_cache: HashMap::new() }), data_fix_type: type_, target_version }
    }

    pub fn storage_info(&self) -> &RegionStorageInfo {
        &self.info
    }

    pub fn update_chunk(&self, pos: ChunkPos, data_fix_context: &Nbt, fixer: impl FnOnce(Nbt) -> Nbt) -> io::Result<()> {
        let mut storage = self.storage.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
        let maybe_tag = storage.read(pos).inspect_err(|e| warn!("Failed to read chunk [{}, {}]\n{e}", pos.x, pos.z))?;
        let Some(tag) = maybe_tag else { return Ok(()) };
        let tag = upgrade_chunk_tag(tag, -1, Some(data_fix_context), self.target_version, self.data_fix_type);
        let tag = fixer(tag);
        storage.write(pos, &tag).inspect_err(|e| error!("Failed to store chunk [{}, {}]\n{e}", pos.x, pos.z))
    }
}

impl AutoCloseable for ChunkNbt {
    fn close(&self) -> io::Result<()> {
        self.storage.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).close()
    }
}
