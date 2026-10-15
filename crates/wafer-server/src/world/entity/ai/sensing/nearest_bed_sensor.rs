use std::collections::HashMap;

use wafer_core::random::RandomSource;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::acquire_poi;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;

const CACHE_TIMEOUT: i64 = 40;
const BATCH_SIZE: i32 = 5;
pub const RATE: i32 = 20;

#[derive(Default)]
pub struct NearestBedSensor {
    batch_cache: HashMap<i64, i64>,
    tried_count: i32,
    last_update: i64,
}

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::NEAREST_BED.erase()]
}

impl NearestBedSensor {
    pub fn do_tick(&mut self, level: &mut ServerLevel, body: EntityId) {
        if !level.is_baby(body) {
            return;
        }
        self.tried_count = 0;
        self.last_update = level.game_time + level.random.next_int_bounded(20) as i64;
        let center = level.entity(body).block_position();
        let (batch_cache, tried_count, last_update) = (&mut self.batch_cache, &mut self.tried_count, self.last_update);
        let pois = level.chunks.poi_manager.find_all_with_type(
            &|poi_type| poi_type == PoiType::Home,
            |pos| {
                let key = pos.as_long();
                if batch_cache.contains_key(&key) {
                    return false;
                }
                *tried_count += 1;
                if *tried_count >= BATCH_SIZE {
                    return false;
                }
                batch_cache.insert(key, last_update + CACHE_TIMEOUT);
                true
            },
            center,
            48,
            Occupancy::Any,
        );
        let path = acquire_poi::find_path_to_pois(level, body, &pois);
        match path.filter(|path| path.can_reach()) {
            Some(path) => {
                let target_pos = path.get_target();
                if level.chunks.poi_manager.get_type(target_pos).is_some() {
                    level.get_brain_mut(body).set_memory(memory_module_type::NEAREST_BED, target_pos);
                }
            }
            None => {
                if self.tried_count < BATCH_SIZE {
                    let last_update = self.last_update;
                    self.batch_cache.retain(|_, value| *value >= last_update);
                }
            }
        }
    }
}
