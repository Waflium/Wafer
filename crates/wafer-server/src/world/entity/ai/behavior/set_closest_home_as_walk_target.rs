use std::collections::HashMap;

use wafer_core::random::RandomSource;

use super::acquire_poi;
use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;

const CACHE_TIMEOUT: i64 = 40;
const BATCH_SIZE: i32 = 5;
const RATE: i64 = 20;
const OK_DISTANCE_SQR: f64 = 4.0;

pub fn create(speed_modifier: f32) -> OneShot {
    let mut batch_cache: HashMap<i64, i64> = HashMap::new();
    let mut last_update: i64 = 0;
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        i.absent(memory_module_type::HOME);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.game_time - last_update < RATE {
                return false;
            }
            let body_pos = level.entity(body).block_position();
            let is_home = |poi_type: PoiType| poi_type == PoiType::Home;
            let closest = level.chunks.poi_manager.find_closest(&is_home, |_| true, body_pos, 48, Occupancy::Any);
            if closest.is_none_or(|closest| closest.dist_sqr(body_pos) <= OK_DISTANCE_SQR) {
                return false;
            }
            let mut tried_count = 0;
            last_update = level.game_time + level.random.next_int_bounded(20) as i64;
            let current_update = last_update;
            let pois = level.chunks.poi_manager.find_all_with_type(
                &is_home,
                |pos| {
                    let key = pos.as_long();
                    if batch_cache.contains_key(&key) {
                        return false;
                    }
                    tried_count += 1;
                    if tried_count >= BATCH_SIZE {
                        return false;
                    }
                    batch_cache.insert(key, current_update + CACHE_TIMEOUT);
                    true
                },
                body_pos,
                48,
                Occupancy::Any,
            );
            let path = acquire_poi::find_path_to_pois(level, body, &pois);
            match path.filter(|path| path.can_reach()) {
                Some(path) => {
                    let target_pos = path.get_target();
                    if level.chunks.poi_manager.get_type(target_pos).is_some() {
                        walk_target.set(level, body, WalkTarget::of_block_pos(target_pos, speed_modifier, 1));
                    }
                }
                None => {
                    if tried_count < BATCH_SIZE {
                        batch_cache.retain(|_, value| *value >= current_update);
                    }
                }
            }
            true
        }
    })
}
