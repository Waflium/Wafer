pub mod chunk_io_error_reporter;
pub mod chunk_stream;
pub mod entity_storage;
pub mod lz4_block_input_stream;
pub mod region_file_version;
pub mod region_storage_info;
pub mod section_storage;
pub mod simple_region_storage;
pub mod storage_exception;

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use wafer_core::ChunkPos;
use wafer_protocol::nbt::Nbt;
use wafer_protocol::{Reader, Writer};

use region_file_version::RegionFileVersion;

#[cfg(windows)]
fn dsync(options: &mut OpenOptions) {
    use std::os::windows::fs::OpenOptionsExt;
    const FILE_FLAG_WRITE_THROUGH: u32 = 0x8000_0000;
    options.custom_flags(FILE_FLAG_WRITE_THROUGH);
}

#[cfg(unix)]
fn dsync(options: &mut OpenOptions) {
    use std::os::unix::fs::OpenOptionsExt;
    options.custom_flags(libc::O_DSYNC);
}

const SECTOR_BYTES: u64 = 4096;
const EXTERNAL_STREAM_FLAG: u8 = 128;

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_owned())
}

const OLD_CHUNK_DATA_VERSION: i32 = 4997;

fn is_old_chunk(nbt: &Nbt) -> bool {
    !matches!(nbt.get("DataVersion"), Some(Nbt::Int(data_version)) if *data_version >= OLD_CHUNK_DATA_VERSION) || matches!(nbt.get("blending_data"), Some(Nbt::Compound(_)))
}

fn chunk_stream_failure(version: RegionFileVersion, data: &[u8], mode: chunk_stream::Mode, caller: &'static str) -> io::Result<Vec<u8>> {
    let kind = match version {
        RegionFileVersion::Gzip => chunk_stream::StreamKind::Gzip,
        RegionFileVersion::Deflate => chunk_stream::StreamKind::Deflate,
        RegionFileVersion::None => chunk_stream::StreamKind::None,
        RegionFileVersion::Lz4 => chunk_stream::StreamKind::Lz4,
    };
    chunk_stream::simulate(data, kind, mode).map_err(|(mut throwable, constructing)| {
        throwable.push_frame(if constructing { storage_exception::construct_stream_caller(caller) } else { caller });
        io::Error::new(io::ErrorKind::InvalidData, throwable)
    })
}

pub fn decode_nbt(bytes: &[u8]) -> io::Result<Nbt> {
    let (_, nbt) = Nbt::read_named(&mut Reader::new(bytes)).map_err(|e| invalid(&e.to_string()))?;
    Ok(nbt)
}

pub fn decode_compressed_nbt(bytes: &[u8]) -> Result<Nbt, String> {
    if bytes.len() < 2 {
        return Err("java.io.EOFException".into());
    }
    if !bytes.starts_with(&[0x1f, 0x8b]) {
        return Err("java.util.zip.ZipException: Not in GZIP format".into());
    }
    let mut decompressed = Vec::new();
    flate2::read::GzDecoder::new(bytes).read_to_end(&mut decompressed).map_err(|_| "java.io.EOFException: Unexpected end of ZLIB input stream".to_string())?;
    decode_nbt(&decompressed).map_err(|e| format!("java.io.IOException: {e}"))
}

pub fn encode_nbt(nbt: &Nbt) -> Vec<u8> {
    let mut w = Writer::new();
    nbt.write_named("", &mut w);
    for fault in w.faults() {
        error!("Failed to write NBT String\n{}", fault.lines().last().unwrap_or_default());
    }
    w.into_inner()
}

#[derive(Default)]
struct RegionBitmap {
    used: Vec<bool>,
}

impl RegionBitmap {
    fn force(&mut self, position: usize, size: usize) {
        if self.used.len() < position + size {
            self.used.resize(position + size, false);
        }
        self.used[position..position + size].fill(true);
    }

    fn free(&mut self, position: usize, size: usize) {
        let end = (position + size).min(self.used.len());
        if position < end {
            self.used[position..end].fill(false);
        }
    }

    fn allocate(&mut self, size: usize) -> usize {
        let mut current = 0;
        loop {
            let free_start = (current..).find(|&i| !self.used.get(i).copied().unwrap_or(false)).unwrap();
            let free_end = (free_start..self.used.len()).find(|&i| self.used[i]);
            match free_end {
                Some(end) if end - free_start < size => current = end,
                _ => {
                    self.force(free_start, size);
                    return free_start;
                }
            }
        }
    }
}

pub trait ExternalChunkFiles: Send {
    fn read(&self, pos: ChunkPos) -> io::Result<Vec<u8>>;
    fn write(&self, pos: ChunkPos, data: &[u8]) -> io::Result<()>;
    fn remove(&self, pos: ChunkPos) -> io::Result<()>;
}

pub struct RegionFile {
    path: PathBuf,
    file: File,
    offsets: [u32; 1024],
    timestamps: [u32; 1024],
    used: RegionBitmap,
    version: RegionFileVersion,
    external: Option<Box<dyn ExternalChunkFiles>>,
    info: Option<region_storage_info::RegionStorageInfo>,
}

impl RegionFile {
    #[cfg(test)]
    pub fn open(path: &Path, sync: bool) -> io::Result<RegionFile> {
        fs::create_dir_all(path.parent().unwrap())?;
        RegionFile::from_file(path, RegionFile::open_file(path, sync)?, None)
    }

    fn open_file(path: &Path, sync: bool) -> io::Result<File> {
        let mut options = OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        if sync {
            dsync(&mut options);
        }
        options.open(path)
    }

    pub fn from_file(path: &Path, mut file: File, external: Option<Box<dyn ExternalChunkFiles>>) -> io::Result<RegionFile> {
        let size = file.metadata()?.len();
        let mut header = vec![0u8; 2 * SECTOR_BYTES as usize];
        let start = crate::rfr::ticks();
        let read = file.read(&mut header)?;
        crate::rfr::io::file_read(path, start, read as i64, read == 0);
        if read != 0 && read != header.len() {
            warn!("Region file {} has truncated header: {read}", path.display());
        }
        let mut region = RegionFile {
            path: path.to_owned(),
            file,
            offsets: [0; 1024],
            timestamps: [0; 1024],
            used: RegionBitmap::default(),
            version: RegionFileVersion::selected(),
            external,
            info: None,
        };
        region.used.force(0, 2);
        for i in 0..1024 {
            region.offsets[i] = u32::from_be_bytes(header[i * 4..i * 4 + 4].try_into().unwrap());
            region.timestamps[i] = u32::from_be_bytes(header[4096 + i * 4..4096 + i * 4 + 4].try_into().unwrap());
            let offset = region.offsets[i];
            if offset == 0 {
                continue;
            }
            let (sector, count) = ((offset >> 8) as usize, (offset & 0xFF) as usize);
            if sector < 2 {
                warn!("Region file {} has invalid sector at index: {i}; sector {sector} overlaps with header", path.display());
                region.offsets[i] = 0;
            } else if count == 0 {
                warn!("Region file {} has an invalid sector at index: {i}; size has to be > 0", path.display());
                region.offsets[i] = 0;
            } else if sector as u64 * SECTOR_BYTES > size {
                warn!("Region file {} has an invalid sector at index: {i}; sector {sector} is out of bounds", path.display());
                region.offsets[i] = 0;
            } else {
                region.used.force(sector, count);
            }
        }
        Ok(region)
    }

    fn offset_index(pos: ChunkPos) -> usize {
        ((pos.x & 31) + (pos.z & 31) * 32) as usize
    }

    fn external_path(&self, pos: ChunkPos) -> PathBuf {
        self.path.with_file_name(format!("c.{}.{}.mcc", pos.x, pos.z))
    }

    pub fn read(&mut self, pos: ChunkPos) -> io::Result<Option<Nbt>> {
        let Some((version, data)) = self.chunk_stream_data(pos)? else { return Ok(None) };
        match version.decompress(&data).and_then(|bytes| decode_nbt(&bytes)) {
            Ok(nbt) if matches!(nbt, Nbt::Compound(_)) => Ok(Some(nbt)),
            _ => chunk_stream_failure(version, &data, chunk_stream::Mode::Read, storage_exception::READ_CHUNK_STREAM).and_then(|prefix| decode_nbt(&prefix).map(Some)),
        }
    }

    pub fn scan_structures(&mut self, pos: ChunkPos) -> io::Result<Option<Nbt>> {
        let Some((version, data)) = self.chunk_stream_data(pos)? else { return Ok(None) };
        match version.decompress(&data).and_then(|bytes| decode_nbt(&bytes)) {
            Ok(nbt) => Ok(Some(nbt)),
            Err(_) => {
                let prefix = chunk_stream_failure(version, &data, chunk_stream::Mode::Scan(chunk_stream::ScanKind::Structures), storage_exception::SCAN_CHUNK_STREAM)?;
                if prefix.first() == Some(&10) { decode_nbt(&prefix).map(Some) } else { Ok(None) }
            }
        }
    }

    pub fn scan(&mut self, pos: ChunkPos) -> io::Result<bool> {
        let Some((version, data)) = self.chunk_stream_data(pos)? else { return Ok(false) };
        match version.decompress(&data).and_then(|bytes| decode_nbt(&bytes)) {
            Ok(nbt) => Ok(is_old_chunk(&nbt)),
            Err(_) => {
                let prefix = chunk_stream_failure(version, &data, chunk_stream::Mode::Scan(chunk_stream::ScanKind::OldData), storage_exception::SCAN_CHUNK_STREAM)?;
                if prefix.first() == Some(&10) { decode_nbt(&prefix).map(|nbt| is_old_chunk(&nbt)) } else { Ok(false) }
            }
        }
    }

    fn chunk_stream_data(&mut self, pos: ChunkPos) -> io::Result<Option<(RegionFileVersion, Vec<u8>)>> {
        let offset = self.offsets[Self::offset_index(pos)];
        if offset == 0 {
            return Ok(None);
        }
        let (sector, count) = ((offset >> 8) as u64, (offset & 0xFF) as u64);
        self.file.seek(SeekFrom::Start(sector * SECTOR_BYTES))?;
        let mut sectors = vec![0u8; (count * SECTOR_BYTES) as usize];
        let start = crate::rfr::ticks();
        let read = self.file.read(&mut sectors)?;
        crate::rfr::io::file_read(&self.path, start, read as i64, read == 0);
        if read < 5 {
            error!("Chunk {pos} header is truncated: expected {} but read {read}", sectors.len());
            return Ok(None);
        }
        let length = i32::from_be_bytes(sectors[..4].try_into().unwrap());
        let version = sectors[4];
        if length == 0 {
            warn!("Chunk {pos} is allocated, but stream is missing");
            return Ok(None);
        }
        let stream_length = length.wrapping_sub(1);
        let data = if version & EXTERNAL_STREAM_FLAG != 0 {
            if stream_length != 0 {
                warn!("Chunk has both internal and external streams");
            }
            match &self.external {
                Some(external) => match external.read(pos) {
                    Ok(data) => data,
                    Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
                    Err(e) => return Err(e),
                },
                None => {
                    let external_file = self.external_path(pos);
                    if !external_file.is_file() {
                        error!("External chunk path {} is not file", external_file.display());
                        return Ok(None);
                    }
                    fs::read(external_file)?
                }
            }
        } else if stream_length > (read - 5) as i32 {
            error!("Chunk {pos} stream is truncated: expected {stream_length} but read {}", read - 5);
            return Ok(None);
        } else if stream_length < 0 {
            error!("Declared size {length} of chunk {pos} is negative");
            return Ok(None);
        } else {
            if let Some(info) = &self.info {
                crate::util::profiling::rfr::rust_profiler::instance().on_region_file_read(info, pos, self.version.id() as i32, stream_length);
            }
            sectors[5..5 + stream_length as usize].to_vec()
        };
        let compression = version & !EXTERNAL_STREAM_FLAG;
        if compression == 127 {
            let mut raw = data.as_slice();
            let mut name_length = [0u8; 2];
            raw.read_exact(&mut name_length)?;
            let mut name = vec![0u8; u16::from_be_bytes(name_length) as usize];
            raw.read_exact(&mut name)?;
            let name = String::from_utf8_lossy(&name);
            match crate::resources::identifier::read(&name) {
                Ok(id) => error!("Unrecognized custom compression {id}"),
                Err(_) => error!("Invalid custom compression id {name}"),
            }
            return Ok(None);
        }
        let Some(compression) = RegionFileVersion::from_id(compression) else {
            error!("Chunk {pos} has invalid chunk stream version {compression}");
            return Ok(None);
        };
        Ok(Some((compression, data)))
    }

    pub fn write(&mut self, pos: ChunkPos, nbt: &Nbt) -> io::Result<()> {
        let compressed = self.version.compress(&encode_nbt(nbt))?;
        if let Some(info) = &self.info {
            crate::util::profiling::rfr::rust_profiler::instance().on_region_file_write(info, pos, self.version.id() as i32, compressed.len() as i32 + 1);
        }
        let mut record = Vec::with_capacity(compressed.len() + 5);
        record.extend(((compressed.len() + 1) as u32).to_be_bytes());
        record.push(self.version.id());
        record.extend(&compressed);

        let index = Self::offset_index(pos);
        let old = self.offsets[index];
        let mut sectors_needed = (record.len() as u64).div_ceil(SECTOR_BYTES) as usize;
        let external = sectors_needed >= 256;
        let new_sector;
        if external {
            warn!("Saving oversized chunk {pos} ({} bytes}} to external file {}", record.len(), self.external_path(pos).display());
            sectors_needed = 1;
            new_sector = self.used.allocate(sectors_needed);
            let mut stub = Vec::from(1u32.to_be_bytes());
            stub.push(self.version.id() | EXTERNAL_STREAM_FLAG);
            match &self.external {
                Some(external) => {
                    external.write(pos, &compressed)?;
                    self.file.seek(SeekFrom::Start(new_sector as u64 * SECTOR_BYTES))?;
                    self.write_all(&stub)?;
                }
                None => {
                    let tmp = self.external_path(pos).with_extension("tmp");
                    fs::write(&tmp, &compressed)?;
                    self.file.seek(SeekFrom::Start(new_sector as u64 * SECTOR_BYTES))?;
                    self.write_all(&stub)?;
                    fs::rename(tmp, self.external_path(pos))?;
                }
            }
        } else {
            new_sector = self.used.allocate(sectors_needed);
            self.file.seek(SeekFrom::Start(new_sector as u64 * SECTOR_BYTES))?;
            self.write_all(&record)?;
            self.remove_external_file(pos);
        }
        self.offsets[index] = (new_sector as u32) << 8 | sectors_needed as u32;
        self.timestamps[index] = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
        self.write_header()?;
        if old != 0 {
            self.used.free((old >> 8) as usize, (old & 0xFF) as usize);
        }
        Ok(())
    }

    pub fn clear(&mut self, pos: ChunkPos) -> io::Result<()> {
        let index = Self::offset_index(pos);
        let offset = self.offsets[index];
        if offset != 0 {
            self.offsets[index] = 0;
            self.timestamps[index] = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as u32);
            self.write_header()?;
            self.remove_external_file(pos);
            self.used.free((offset >> 8) as usize, (offset & 0xFF) as usize);
        }
        Ok(())
    }

    fn remove_external_file(&self, pos: ChunkPos) {
        match &self.external {
            Some(external) => {
                let _ = external.remove(pos);
            }
            None => {
                let _ = fs::remove_file(self.external_path(pos));
            }
        }
    }

    pub fn close(&mut self) -> io::Result<()> {
        self.pad_to_full_sector()?;
        self.file.sync_all()
    }

    fn write_header(&mut self) -> io::Result<()> {
        let mut header = Vec::with_capacity(8192);
        self.offsets.iter().for_each(|o| header.extend(o.to_be_bytes()));
        self.timestamps.iter().for_each(|t| header.extend(t.to_be_bytes()));
        self.file.seek(SeekFrom::Start(0))?;
        self.write_all(&header)
    }

    fn write_all(&mut self, bytes: &[u8]) -> io::Result<()> {
        let start = crate::rfr::ticks();
        self.file.write_all(bytes)?;
        if crate::rfr::runtime::FILE_WRITE.is_enabled() {
            crate::rfr::io::file_write(Some(&crate::rfr::io::absolute_path_string(&self.path)), start, bytes.len() as i64);
        }
        Ok(())
    }

    fn pad_to_full_sector(&mut self) -> io::Result<()> {
        let size = self.file.metadata()?.len();
        let padded = size.div_ceil(SECTOR_BYTES) * SECTOR_BYTES;
        if padded != size {
            self.file.seek(SeekFrom::Start(padded - 1))?;
            self.file.write_all(&[0])?;
        }
        Ok(())
    }
}

pub struct RegionStorage {
    dir: PathBuf,
    sync: bool,
    info: Option<region_storage_info::RegionStorageInfo>,
    open: HashMap<(i32, i32), RegionFile>,
}

impl RegionStorage {
    pub fn new(dir: PathBuf, sync: bool) -> RegionStorage {
        RegionStorage { dir, sync, info: None, open: HashMap::new() }
    }

    pub fn with_info(mut self, info: region_storage_info::RegionStorageInfo) -> RegionStorage {
        self.info = Some(info);
        self
    }

    fn region_path(&self, pos: ChunkPos) -> PathBuf {
        self.dir.join(format!("r.{}.{}.mca", pos.x >> 5, pos.z >> 5))
    }

    fn region(&mut self, pos: ChunkPos, callers: &[&'static str]) -> io::Result<&mut RegionFile> {
        let key = (pos.x >> 5, pos.z >> 5);
        if !self.open.contains_key(&key) {
            let path = self.region_path(pos);
            let directory = if self.dir.exists() { self.dir.canonicalize().map_err(|e| storage_exception::create_directories_safe_failure(e, &self.dir, callers))? } else { self.dir.clone() };
            fs::create_dir_all(&directory).map_err(|e| storage_exception::create_directories_safe_failure(e, &directory, callers))?;
            let file = RegionFile::open_file(&path, self.sync).map_err(|e| storage_exception::open_region_file_failure(e, &path, self.sync, callers))?;
            let mut region = RegionFile::from_file(&path, file, None)?;
            region.info = self.info.clone();
            self.open.insert(key, region);
        }
        Ok(self.open.get_mut(&key).unwrap())
    }

    pub fn read(&mut self, pos: ChunkPos) -> io::Result<Option<Nbt>> {
        if !self.open.contains_key(&(pos.x >> 5, pos.z >> 5)) && !self.region_path(pos).is_file() {
            return Ok(None);
        }
        self.region(pos, &storage_exception::READ_GET_REGION_FILE)?.read(pos)
    }

    pub fn scan_chunk(&mut self, pos: ChunkPos) -> io::Result<bool> {
        if !self.open.contains_key(&(pos.x >> 5, pos.z >> 5)) && !self.region_path(pos).is_file() {
            return Ok(false);
        }
        self.region(pos, &storage_exception::SCAN_CHUNK_GET_REGION_FILE)?.scan(pos)
    }

    pub fn scan_structures(&mut self, pos: ChunkPos) -> io::Result<Option<Nbt>> {
        if !self.open.contains_key(&(pos.x >> 5, pos.z >> 5)) && !self.region_path(pos).is_file() {
            return Ok(None);
        }
        self.region(pos, &storage_exception::SCAN_CHUNK_GET_REGION_FILE)?.scan_structures(pos)
    }

    pub fn write(&mut self, pos: ChunkPos, nbt: &Nbt) -> io::Result<()> {
        self.region(pos, &storage_exception::WRITE_GET_REGION_FILE)?.write(pos, nbt)
    }

    pub fn clear(&mut self, pos: ChunkPos) -> io::Result<()> {
        self.region(pos, &storage_exception::WRITE_GET_REGION_FILE)?.clear(pos)
    }

    pub fn flush(&mut self) -> io::Result<()> {
        for region in self.open.values_mut() {
            region.pad_to_full_sector()?;
            region.file.sync_data()?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn region_round_trip() {
        let dir = std::env::temp_dir().join(format!("wafer-region-{}", std::process::id()));
        let mut store = RegionStorage::new(dir.clone(), false);
        let small = Nbt::Compound(vec![("a".into(), Nbt::Int(1))]);
        let large = Nbt::Compound(vec![("big".into(), Nbt::LongArray((0..3000).map(|i| i * 7919).collect()))]);
        store.write(ChunkPos::new(0, 0), &small).unwrap();
        store.write(ChunkPos::new(-1, 33), &large).unwrap();
        store.write(ChunkPos::new(0, 0), &large).unwrap();
        store.flush().unwrap();
        let mut reopened = RegionStorage::new(dir.clone(), false);
        assert_eq!(reopened.read(ChunkPos::new(0, 0)).unwrap(), Some(large.clone()));
        assert_eq!(reopened.read(ChunkPos::new(-1, 33)).unwrap(), Some(large));
        assert_eq!(reopened.read(ChunkPos::new(5, 5)).unwrap(), None);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unreadable_chunks_read_as_missing() {
        let dir = std::env::temp_dir().join(format!("wafer-region-unreadable-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut store = RegionStorage::new(dir.clone(), false);
        let tag = Nbt::Compound(vec![("a".into(), Nbt::Int(1))]);
        let corruptions: [(i32, fn(&mut [u8])); 6] = [
            (0, |chunk| chunk[..4].copy_from_slice(&0i32.to_be_bytes())),
            (1, |chunk| chunk[..4].copy_from_slice(&(-5i32).to_be_bytes())),
            (2, |chunk| chunk[..4].copy_from_slice(&i32::MAX.to_be_bytes())),
            (3, |chunk| chunk[4] = 9),
            (4, |chunk| chunk[4] = 2 | EXTERNAL_STREAM_FLAG),
            (5, |chunk| {
                chunk[4] = 127;
                chunk[5..7].copy_from_slice(&7u16.to_be_bytes());
                chunk[7..14].copy_from_slice(b"foo:bar");
            }),
        ];
        for (x, _) in corruptions {
            store.write(ChunkPos::new(x, 0), &tag).unwrap();
        }
        store.flush().unwrap();
        drop(store);
        let path = dir.join("r.0.0.mca");
        let mut bytes = fs::read(&path).unwrap();
        for (x, corrupt) in corruptions {
            let sector = u32::from_be_bytes(bytes[x as usize * 4..x as usize * 4 + 4].try_into().unwrap()) >> 8;
            corrupt(&mut bytes[sector as usize * 4096..]);
        }
        fs::write(&path, bytes).unwrap();
        let mut store = RegionStorage::new(dir.clone(), false);
        for (x, _) in corruptions {
            assert_eq!(store.read(ChunkPos::new(x, 0)).unwrap(), None, "chunk {x}");
        }
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupt_chunk_streams_fail_with_java_exceptions_and_traces() {
        let dir = std::env::temp_dir().join(format!("wafer-region-corrupt-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        let mut store = RegionStorage::new(dir.clone(), false);
        store.write(ChunkPos::new(0, 0), &Nbt::Compound(vec![("a".into(), Nbt::Int(1))])).unwrap();
        store.flush().unwrap();
        drop(store);
        let path = dir.join("r.0.0.mca");
        let mut bytes = fs::read(&path).unwrap();
        let sector = (u32::from_be_bytes(bytes[0..4].try_into().unwrap()) >> 8) as usize * 4096;
        bytes[sector + 5..sector + 12].fill(0xab);
        fs::write(&path, bytes).unwrap();
        let mut store = RegionStorage::new(dir.clone(), false);
        let failure = crate::util::java_throwable::JavaThrowable::from_io_error(&store.read(ChunkPos::new(0, 0)).unwrap_err());
        assert_eq!(failure.description, "java.util.zip.ZipException: incorrect header check");
        assert_eq!(failure.stack_trace.first().copied(), Some("java.base/java.util.zip.InflaterInputStream.read(InflaterInputStream.java:214)"));
        assert_eq!(failure.stack_trace.last().copied(), Some(storage_exception::READ_CHUNK_STREAM));
        let scan_failure = crate::util::java_throwable::JavaThrowable::from_io_error(&store.scan_chunk(ChunkPos::new(0, 0)).unwrap_err());
        assert_eq!(scan_failure.description, failure.description);
        assert_eq!(scan_failure.stack_trace.last().copied(), Some(storage_exception::SCAN_CHUNK_STREAM));
        assert_eq!(store.scan_chunk(ChunkPos::new(1, 0)).unwrap(), false);
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn unopenable_region_files_fail_like_file_channel_open() {
        let dir = std::env::temp_dir().join(format!("wafer-region-denied-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join("r.0.0.mca"), []).unwrap();
        let mut permissions = fs::metadata(dir.join("r.0.0.mca")).unwrap().permissions();
        permissions.set_readonly(true);
        fs::set_permissions(dir.join("r.0.0.mca"), permissions).unwrap();
        let mut store = RegionStorage::new(dir.clone(), true);
        let failure = crate::util::java_throwable::JavaThrowable::from_io_error(&store.read(ChunkPos::new(0, 0)).unwrap_err());
        assert!(failure.description.starts_with("java.nio.file."), "{}", failure.description);
        assert!(failure.stack_trace.contains(&"net.minecraft.world.level.chunk.storage.RegionFile.<init>(RegionFile.java:132)"));
        assert_eq!(failure.stack_trace.last().copied(), Some("net.minecraft.world.level.chunk.storage.RegionFileStorage.read(RegionFileStorage.java:80)"));
        let mut permissions = fs::metadata(dir.join("r.0.0.mca")).unwrap().permissions();
        permissions.set_readonly(false);
        fs::set_permissions(dir.join("r.0.0.mca"), permissions).unwrap();
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn first_fit_reuses_freed_sectors() {
        let mut bitmap = RegionBitmap::default();
        bitmap.force(0, 2);
        assert_eq!(bitmap.allocate(3), 2);
        assert_eq!(bitmap.allocate(1), 5);
        bitmap.free(2, 3);
        assert_eq!(bitmap.allocate(2), 2);
        assert_eq!(bitmap.allocate(2), 6);
    }
}
