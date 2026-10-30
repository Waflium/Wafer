use std::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;

use wafer_data::WORLD_VERSION;
use wafer_protocol::nbt::Nbt;

use super::write_dat;
use crate::util::codec::DecodeResult;
use crate::util::datafix::data_fix_types::DataFixTypes;
use crate::world::entity::EntityId;
use crate::world::level::saveddata::maps::map_id::MapId;
use crate::world::level::saveddata::maps::map_index::{self, MapIndex};
use crate::world::level::saveddata::maps::map_item_saved_data::MapItemSavedData;
use crate::world::level::saveddata::wandering_trader_data::{self, WanderingTraderData};

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
