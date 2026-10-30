use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::{Arc, Condvar, Mutex};

use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use super::write_dat;
use crate::util::codec::DecodeResult;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::world::entity::EntityId;
use crate::world::level::chunk::storage::WRITE_COMPRESSED_PATH;
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::level::saveddata::maps::map_index::{self, MapIndex};
use crate::world::level::saveddata::maps::map_item_saved_data::MapItemSavedData;
use crate::world::level::saveddata::wandering_trader_data::{self, WanderingTraderData};

thread_local! {
    static DIRTY_TAGS: RefCell<Option<Vec<(PathBuf, Nbt)>>> = const { RefCell::new(None) };
}

pub fn collect_dirty_tags_to_save(collect: impl FnOnce()) -> Vec<(PathBuf, Nbt)> {
    let outer = DIRTY_TAGS.with(|tags| tags.replace(Some(Vec::new())));
    collect();
    DIRTY_TAGS.with(|tags| tags.replace(outer)).unwrap_or_default()
}

pub(super) fn collect_dirty_tag(path: &Path, tag: &Nbt) -> bool {
    DIRTY_TAGS.with(|tags| match tags.borrow_mut().as_mut() {
        Some(tags) => {
            tags.push((path.to_path_buf(), tag.clone()));
            true
        }
        None => false,
    })
}

const TRY_WRITE_CALLERS: [&str; 3] = [WRITE_COMPRESSED_PATH[0], WRITE_COMPRESSED_PATH[1], "net.minecraft.world.level.storage.SavedDataStorage.tryWrite(SavedDataStorage.java:207)"];
const SCHEDULE_SAVE_EACH: &str = "net.minecraft.world.level.storage.SavedDataStorage.lambda$scheduleSave$5(SavedDataStorage.java:175)";
const SCHEDULE_SAVE_PARTITION: &str = "net.minecraft.world.level.storage.SavedDataStorage.lambda$scheduleSave$1(SavedDataStorage.java:165)";
const IO_POOL_TAIL: [&str; 4] = [
    "java.base/java.util.concurrent.CompletableFuture$AsyncRun.run(CompletableFuture.java:1825)",
    "java.base/java.util.concurrent.ThreadPoolExecutor.runWorker(ThreadPoolExecutor.java:1090)",
    "java.base/java.util.concurrent.ThreadPoolExecutor$Worker.run(ThreadPoolExecutor.java:614)",
    "java.base/java.lang.Thread.run(Thread.java:1474)",
];

#[derive(Default)]
struct PendingWrite {
    remaining: Mutex<usize>,
    done: Condvar,
}

impl PendingWrite {
    fn complete_one(&self) {
        let mut remaining = self.remaining.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        *remaining -= 1;
        if *remaining == 0 {
            self.done.notify_all();
        }
    }

    fn is_done(&self) -> bool {
        *self.remaining.lock().unwrap_or_else(std::sync::PoisonError::into_inner) == 0
    }

    fn join(&self) {
        let remaining = self.remaining.lock().unwrap_or_else(std::sync::PoisonError::into_inner);
        drop(self.done.wait_while(remaining, |remaining| *remaining > 0).unwrap_or_else(std::sync::PoisonError::into_inner));
    }
}

#[derive(Default)]
pub struct SavedDataWrites {
    pending_write: Option<Arc<PendingWrite>>,
}

impl SavedDataWrites {
    pub fn schedule_save(&mut self, tags_to_save: Vec<(PathBuf, Nbt)>) -> bool {
        if tags_to_save.is_empty() {
            return false;
        }
        let threads = crate::util::max_allowed_executor_threads();
        let partitioned = tags_to_save.len() > threads;
        let bucket_size = if partitioned { tags_to_save.len().div_ceil(threads) } else { 1 };
        let mut tasks: Vec<Vec<(PathBuf, Nbt)>> = Vec::new();
        for entry in tags_to_save {
            match tasks.last_mut() {
                Some(task) if task.len() < bucket_size => task.push(entry),
                _ => tasks.push(vec![entry]),
            }
        }
        let pending = Arc::new(PendingWrite { remaining: Mutex::new(tasks.len()), done: Condvar::new() });
        let previous = self.pending_write.replace(pending.clone());
        let start = move || {
            for task in tasks {
                let pending = pending.clone();
                crate::util::io_pool().execute(move || {
                    let mut frames = vec![if partitioned { SCHEDULE_SAVE_PARTITION } else { SCHEDULE_SAVE_EACH }];
                    frames.extend(IO_POOL_TAIL);
                    let _frames = crate::server::main_thread_frames::enter_complete(frames);
                    for (path, tag) in &task {
                        try_write(path, tag);
                    }
                    pending.complete_one();
                });
            }
        };
        match previous {
            Some(previous) if !previous.is_done() => crate::util::io_pool().execute(move || {
                previous.join();
                start();
            }),
            _ => start(),
        }
        true
    }

    pub fn save_and_join(&mut self, tags_to_save: Vec<(PathBuf, Nbt)>) {
        if self.schedule_save(tags_to_save)
            && let Some(pending) = &self.pending_write
        {
            pending.join();
        }
    }
}

fn try_write(path: &Path, tag: &Nbt) {
    let written = crate::util::file_util::create_directories_safe(path.parent().expect("saved data directory")).and_then(|()| super::write_compressed(tag, path, &TRY_WRITE_CALLERS));
    if let Err(error) = written {
        let file_name = path.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned());
        error!("Could not save data to {file_name}\n{}", crate::util::java_throwable::JavaThrowable::from_io_error(&error).print_stack_trace().trim_end());
    }
}

pub struct SavedDataStorage {
    data_folder: Option<PathBuf>,
    maps: HashMap<MapId, Option<Rc<RefCell<MapItemSavedData>>>>,
    map_index: Option<Option<MapIndex>>,
    wandering_trader: Option<WanderingTraderData>,
}

impl SavedDataStorage {
    pub fn new(data_folder: Option<PathBuf>) -> SavedDataStorage {
        SavedDataStorage { data_folder, maps: HashMap::new(), map_index: None, wandering_trader: None }
    }

    fn get_data_file(&self, id: &str) -> Option<PathBuf> {
        Some(self.data_folder.as_ref()?.join("minecraft").join(format!("{id}.dat")))
    }

    fn read_saved_data<T>(&self, id: &str, data_fix_type: DataFixTypes, decode: impl FnOnce(&Nbt) -> DecodeResult<T>) -> Option<T> {
        super::read_saved_data(&self.get_data_file(id)?, "Server thread", id, data_fix_type, decode)
    }

    pub fn get_map_data(&mut self, id: MapId) -> Option<Rc<RefCell<MapItemSavedData>>> {
        if !self.maps.contains_key(&id) {
            let data = self.read_saved_data(&id.key(), DataFixTypes::SavedDataMapData, MapItemSavedData::decode).map(|data| Rc::new(RefCell::new(data)));
            self.maps.insert(id, data);
        }
        self.maps[&id].clone()
    }

    pub fn set_map_data(&mut self, id: MapId, mut data: MapItemSavedData) {
        data.set_dirty();
        self.maps.insert(id, Some(Rc::new(RefCell::new(data))));
    }

    pub fn map_index(&mut self) -> &mut MapIndex {
        if self.map_index.is_none() {
            self.map_index = Some(self.read_saved_data(map_index::TYPE, DataFixTypes::SavedDataMapIndex, MapIndex::decode));
        }
        let index = self.map_index.as_mut().unwrap();
        if index.is_none() {
            let mut new_data = MapIndex::default();
            new_data.dirty = true;
            *index = Some(new_data);
        }
        index.as_mut().unwrap()
    }

    pub fn wandering_trader_data(&mut self) -> &mut WanderingTraderData {
        if self.wandering_trader.is_none() {
            let data = self.read_saved_data(wandering_trader_data::TYPE, DataFixTypes::SavedDataWanderingTrader, WanderingTraderData::decode).unwrap_or_else(WanderingTraderData::new_dirty);
            self.wandering_trader = Some(data);
        }
        self.wandering_trader.as_mut().unwrap()
    }

    pub fn player_removed(&self, player: EntityId) {
        for data in self.maps.values().flatten() {
            data.borrow_mut().player_removed(player);
        }
    }

    pub fn save(&mut self) {
        let mut to_save = Vec::new();
        for (id, data) in &self.maps {
            if let Some(data) = data {
                let mut data = data.borrow_mut();
                if data.dirty {
                    to_save.push((id.key(), data.encode()));
                    data.dirty = false;
                }
            }
        }
        if let Some(Some(index)) = &mut self.map_index
            && index.dirty
        {
            to_save.push((map_index::TYPE.to_string(), index.encode()));
            index.dirty = false;
        }
        if let Some(data) = &mut self.wandering_trader
            && data.dirty
        {
            to_save.push((wandering_trader_data::TYPE.to_string(), data.encode()));
            data.dirty = false;
        }
        for (id, data) in to_save {
            let Some(file) = self.get_data_file(&id) else { continue };
            let tag = Nbt::Compound(vec![("data".into(), data), ("DataVersion".into(), Nbt::Int(WORLD_VERSION))].into());
            if write_dat(&file, &tag).is_err() {
                error!("Could not save data to {}", file.file_name().map_or_else(String::new, |name| name.to_string_lossy().into_owned()));
            }
        }
    }
}
