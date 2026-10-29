use std::collections::HashMap;
use std::sync::mpsc::{Receiver, channel};

use wafer_core::ChunkPos;
use wafer_core::fastutil::{Long2ObjectOpenHashMap, LongLinkedOpenHashSet, LongOpenHashSet};
use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use crate::server::level::chunk_workers::{IoReporting, IoWorker};
use crate::util::java_throwable::JavaThrowable;

pub trait SectionData: Default + Send + 'static {
    fn pack(&self) -> Nbt;
    fn unpack(nbt: &Nbt) -> Option<Self>;
}

pub struct PackedChunk<R> {
    sections_by_y: Vec<(i32, R)>,
    version_changed: bool,
}

impl<R: SectionData> PackedChunk<R> {
    fn parse(tag: &Nbt, min_section_y: i32, max_section_y: i32) -> PackedChunk<R> {
        let version_changed = crate::nbt::get_data_version(tag, 1945) < WORLD_VERSION;
        let fixed_tag = super::simple_region_storage::upgrade_chunk_tag_to_current_version(tag, 1945, crate::util::datafix::data_fix_types::DataFixTypes::PoiChunk);
        let sections = fixed_tag.get("Sections");
        let mut sections_by_y = Vec::new();
        for section_y in min_section_y..=max_section_y {
            if let Some(section) = sections.and_then(|s| s.get(&section_y.to_string())).and_then(R::unpack) {
                sections_by_y.push((section_y, section));
            }
        }
        PackedChunk { sections_by_y, version_changed }
    }
}

type LoadResult<R> = Result<Option<PackedChunk<R>>, JavaThrowable>;

enum PendingLoad<R> {
    Reading(Receiver<LoadResult<R>>),
    Done(LoadResult<R>),
}

impl<R> PendingLoad<R> {
    fn get_now(&mut self) -> Option<&mut LoadResult<R>> {
        if let PendingLoad::Reading(receiver) = self {
            let result = match receiver.try_recv() {
                Ok(result) => result,
                Err(std::sync::mpsc::TryRecvError::Empty) => return None,
                Err(std::sync::mpsc::TryRecvError::Disconnected) => Ok(None),
            };
            *self = PendingLoad::Done(result);
        }
        match self {
            PendingLoad::Done(result) => Some(result),
            PendingLoad::Reading(_) => None,
        }
    }

    fn join(&mut self) -> &mut LoadResult<R> {
        if let PendingLoad::Reading(receiver) = self {
            let result = receiver.recv().unwrap_or(Ok(None));
            *self = PendingLoad::Done(result);
        }
        match self {
            PendingLoad::Done(result) => result,
            PendingLoad::Reading(_) => unreachable!(),
        }
    }
}

fn completion_exception(cause: &JavaThrowable) -> ! {
    crate::crash_report::throw_java_exception(super::storage_exception::section_storage_completion_exception(cause))
}

pub fn section_key(x: i32, y: i32, z: i32) -> i64 {
    ((x as i64 & 0x3FFFFF) << 42) | (y as i64 & 0xFFFFF) | ((z as i64 & 0x3FFFFF) << 20)
}

fn section_x(key: i64) -> i32 {
    (key >> 42) as i32
}

fn section_y(key: i64) -> i32 {
    ((key << 44) >> 44) as i32
}

fn section_z(key: i64) -> i32 {
    ((key << 22) >> 42) as i32
}

pub struct SectionStorage<R> {
    io: Option<IoWorker>,
    storage: HashMap<i64, Option<R>>,
    dirty_chunks: LongLinkedOpenHashSet,
    min_section_y: i32,
    max_section_y: i32,
    loaded_chunks: LongOpenHashSet,
    pending_loads: Long2ObjectOpenHashMap<PendingLoad<R>>,
    pub loaded_sections: std::cell::RefCell<Vec<i64>>,
    reporting: Option<IoReporting>,
}

impl<R: SectionData> SectionStorage<R> {
    pub fn new(io: Option<IoWorker>, min_section_y: i32, max_section_y: i32) -> SectionStorage<R> {
        SectionStorage {
            io,
            storage: HashMap::new(),
            dirty_chunks: LongLinkedOpenHashSet::new(),
            min_section_y,
            max_section_y,
            loaded_chunks: LongOpenHashSet::new(),
            pending_loads: Long2ObjectOpenHashMap::new(),
            loaded_sections: std::cell::RefCell::new(Vec::new()),
            reporting: None,
        }
    }

    pub fn set_reporting(&mut self, reporting: IoReporting) {
        if let Some(io) = &self.io {
            io.set_reporting(reporting.clone());
        }
        self.reporting = Some(reporting);
    }

    pub fn section_range(&self) -> (i32, i32) {
        (self.min_section_y, self.max_section_y)
    }

    pub fn tick(&mut self, have_time: &mut dyn FnMut() -> bool) {
        while have_time() {
            let Some(chunk) = self.dirty_chunks.remove_first() else { break };
            self.write_chunk(ChunkPos::unpack(chunk));
        }
        self.unpack_pending_loads();
    }

    fn unpack_pending_loads(&mut self) {
        let mut ready = Vec::new();
        let mut failure = None;
        self.pending_loads.retain(|key, pending| {
            if failure.is_some() {
                return true;
            }
            match pending.get_now() {
                None => true,
                Some(Err(exception)) => {
                    failure = Some(exception.clone());
                    true
                }
                Some(Ok(chunk)) => {
                    ready.push((key, chunk.take()));
                    false
                }
            }
        });
        for (key, chunk) in ready {
            self.unpack_packed_chunk(ChunkPos::unpack(key), chunk);
            self.loaded_chunks.add(key);
        }
        if let Some(exception) = failure {
            completion_exception(&exception);
        }
    }

    pub fn flush_all(&mut self) {
        while let Some(chunk) = self.dirty_chunks.remove_first() {
            self.write_chunk(ChunkPos::unpack(chunk));
        }
    }

    pub fn has_work(&self) -> bool {
        !self.dirty_chunks.is_empty()
    }

    pub fn get(&self, section_pos: i64) -> Option<&Option<R>> {
        self.storage.get(&section_pos)
    }

    pub fn get_mut(&mut self, section_pos: i64) -> Option<&mut Option<R>> {
        self.storage.get_mut(&section_pos)
    }

    pub fn get_or_load(&mut self, section_pos: i64) -> Option<&mut R> {
        if self.outside_stored_range(section_pos) {
            return None;
        }
        if !self.storage.contains_key(&section_pos) {
            self.unpack_chunk(ChunkPos::new(section_x(section_pos), section_z(section_pos)));
        }
        self.storage.get_mut(&section_pos).unwrap_or_else(|| panic!("java.lang.IllegalStateException")).as_mut()
    }

    fn outside_stored_range(&self, section_pos: i64) -> bool {
        let y = section_y(section_pos);
        y < self.min_section_y || y > self.max_section_y
    }

    pub fn get_or_create(&mut self, section_pos: i64) -> &mut R {
        assert!(!self.outside_stored_range(section_pos), "sectionPos out of bounds");
        if self.get_or_load(section_pos).is_none() {
            self.storage.insert(section_pos, Some(R::default()));
        }
        self.storage.get_mut(&section_pos).and_then(Option::as_mut).expect("created section")
    }

    pub fn prefetch(&mut self, chunk_pos: ChunkPos) {
        let key = chunk_pos.pack();
        if self.loaded_chunks.contains(key) || self.pending_loads.contains_key(key) {
            return;
        }
        let receiver = self.try_read(chunk_pos);
        self.pending_loads.insert(key, PendingLoad::Reading(receiver));
    }

    pub fn prefetched_load_failure(&mut self, chunk_pos: ChunkPos) -> Option<JavaThrowable> {
        match self.pending_loads.get_mut(chunk_pos.pack())?.join() {
            Err(exception) => Some(exception.clone()),
            Ok(_) => None,
        }
    }

    fn unpack_chunk(&mut self, chunk_pos: ChunkPos) {
        let key = chunk_pos.pack();
        if !self.loaded_chunks.add(key) {
            return;
        }
        if !self.pending_loads.contains_key(key) {
            let receiver = self.try_read(chunk_pos);
            self.pending_loads.insert(key, PendingLoad::Reading(receiver));
        }
        let chunk = match self.pending_loads.get_mut(key).unwrap().join() {
            Ok(chunk) => chunk.take(),
            Err(exception) => completion_exception(&exception.clone()),
        };
        self.unpack_packed_chunk(chunk_pos, chunk);
        self.pending_loads.remove(key);
    }

    fn try_read(&self, chunk_pos: ChunkPos) -> Receiver<LoadResult<R>> {
        let (sender, receiver) = channel();
        let (min_section_y, max_section_y) = (self.min_section_y, self.max_section_y);
        let reporting = self.reporting.clone();
        match &self.io {
            Some(io) => io.read(chunk_pos, move |read| {
                let chunk = match read {
                    Ok(tag) => crate::util::catch_exception(|| tag.map(|tag| PackedChunk::parse(&tag, min_section_y, max_section_y))).map_err(|message| JavaThrowable::new(message, Vec::new())),
                    Err(e) => {
                        let throwable = JavaThrowable::from_io_error(&e);
                        if throwable.is_io_exception() {
                            error!("Error reading chunk {chunk_pos} data from disk\n{}", throwable.print_stack_trace().trim_end());
                            if let Some(reporting) = &reporting {
                                reporting.reporter.report_chunk_load_failure(&throwable, &reporting.info, chunk_pos);
                            }
                            Ok(None)
                        } else {
                            Err(throwable)
                        }
                    }
                };
                let _ = sender.send(chunk);
            }),
            None => {
                let _ = sender.send(Ok(None));
            }
        }
        receiver
    }

    fn unpack_packed_chunk(&mut self, pos: ChunkPos, packed_chunk: Option<PackedChunk<R>>) {
        match packed_chunk {
            None => {
                for section_y in self.min_section_y..=self.max_section_y {
                    self.storage.insert(section_key(pos.x, section_y, pos.z), None);
                }
            }
            Some(PackedChunk { mut sections_by_y, version_changed }) => {
                for section_y in self.min_section_y..=self.max_section_y {
                    let key = section_key(pos.x, section_y, pos.z);
                    let section = sections_by_y.iter().position(|(y, _)| *y == section_y).map(|i| sections_by_y.swap_remove(i).1);
                    let present = section.is_some();
                    self.storage.insert(key, section);
                    if present {
                        self.loaded_sections.get_mut().push(key);
                        if version_changed {
                            self.set_dirty(key);
                        }
                    }
                }
            }
        }
    }

    fn write_chunk(&mut self, chunk_pos: ChunkPos) {
        let mut sections = Vec::new();
        for section_y in self.min_section_y..=self.max_section_y {
            if let Some(Some(section)) = self.storage.get(&section_key(chunk_pos.x, section_y, chunk_pos.z)) {
                sections.push((section_y.to_string(), section.pack()));
            }
        }
        let tag = Nbt::Compound(vec![("Sections".into(), Nbt::Compound(sections)), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))]);
        if let Some(io) = &self.io {
            io.write(chunk_pos, tag);
        }
    }

    pub fn set_dirty(&mut self, section_pos: i64) {
        if self.storage.get(&section_pos).is_some_and(Option::is_some) {
            self.dirty_chunks.add(ChunkPos::new(section_x(section_pos), section_z(section_pos)).pack());
        } else {
            warn!("No data for position: SectionPos{{x={}, y={}, z={}}}", section_x(section_pos), section_y(section_pos), section_z(section_pos));
        }
    }

    pub fn flush(&mut self, chunk_pos: ChunkPos) {
        if self.dirty_chunks.remove(chunk_pos.pack()) {
            self.write_chunk(chunk_pos);
        }
    }

    pub fn synchronize(&self, flush: bool) {
        if let Some(io) = &self.io {
            let _ = io.synchronize(flush);
        }
    }
}
