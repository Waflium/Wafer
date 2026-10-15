use std::collections::HashMap;

use wafer_core::BlockPos;
use wafer_core::random::RandomSource;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::MemoryModuleType;
use crate::world::entity::ai::village::poi::poi_manager::Occupancy;
use crate::world::entity::ai::village::poi::poi_types::PoiType;
use crate::world::level::pathfinder::path::Path;

pub const SCAN_RATE: i32 = 48;

pub fn create(
    poi_type: impl Fn(PoiType) -> bool + 'static,
    memory_to_acquire: MemoryModuleType<GlobalPos>,
    only_if_adult: bool,
    on_poi_acquisition_event: Option<u8>,
    valid_poi: impl Fn(&mut ServerLevel, BlockPos) -> bool + 'static,
) -> OneShot {
    create_validating(poi_type, memory_to_acquire, memory_to_acquire, only_if_adult, on_poi_acquisition_event, valid_poi)
}

pub fn create_validating(
    poi_type: impl Fn(PoiType) -> bool + 'static,
    memory_to_validate: MemoryModuleType<GlobalPos>,
    memory_to_acquire: MemoryModuleType<GlobalPos>,
    only_if_adult: bool,
    on_poi_acquisition_event: Option<u8>,
    valid_poi: impl Fn(&mut ServerLevel, BlockPos) -> bool + 'static,
) -> OneShot {
    let mut next_scheduled_start: i64 = 0;
    let mut batch_cache: HashMap<i64, JitteredLinearRetry> = HashMap::new();
    behavior_builder::create(|i| {
        if memory_to_validate != memory_to_acquire {
            i.absent(memory_to_validate);
        }
        let to_acquire = i.absent(memory_to_acquire);
        move |level: &mut ServerLevel, body: EntityId, timestamp: i64| {
            if only_if_adult && level.is_baby(body) {
                return false;
            }
            if next_scheduled_start == 0 {
                next_scheduled_start = level.game_time + level.random.next_int_bounded(20) as i64;
                return false;
            }
            if level.game_time < next_scheduled_start {
                return false;
            }
            next_scheduled_start = timestamp + 20 + level.random.next_int_bounded(20) as i64;
            batch_cache.retain(|_, retry| retry.is_still_valid(timestamp));
            let center = level.entity(body).block_position();
            let ServerLevel { chunks, random, .. } = level;
            let found = chunks.poi_manager.find_all_closest_first_with_type(
                &poi_type,
                |pos| match batch_cache.get_mut(&pos.as_long()) {
                    None => true,
                    Some(retry_marker) => {
                        if !retry_marker.should_retry(timestamp) {
                            return false;
                        }
                        retry_marker.mark_attempt(random, timestamp);
                        true
                    }
                },
                center,
                SCAN_RATE,
                Occupancy::HasSpace,
            );
            let mut poi_positions: Vec<(PoiType, BlockPos)> = Vec::new();
            for (ty, pos) in found.into_iter().take(5) {
                if valid_poi(level, pos) {
                    poi_positions.push((ty, pos));
                }
            }
            let path = find_path_to_pois(level, body, &poi_positions);
            match path.filter(Path::can_reach) {
                Some(path) => {
                    let target_pos = path.get_target();
                    if level.chunks.poi_manager.get_type(target_pos).is_some() {
                        level.chunks.poi_manager.take(&poi_type, |_, poi_pos| poi_pos == target_pos, target_pos, 1);
                        let dimension = level.dimension;
                        to_acquire.set(level, body, GlobalPos::of(dimension, target_pos));
                        if let Some(event) = on_poi_acquisition_event {
                            level.broadcast_entity_event(body, event);
                        }
                        batch_cache.clear();
                    }
                }
                None => {
                    for (_, pos) in &poi_positions {
                        batch_cache.entry(pos.as_long()).or_insert_with(|| JitteredLinearRetry::new(&mut level.random, timestamp));
                    }
                }
            }
            true
        }
    })
}

pub fn find_path_to_pois(level: &mut ServerLevel, body: EntityId, pois: &[(PoiType, BlockPos)]) -> Option<Path> {
    if pois.is_empty() {
        return None;
    }
    let mut targets: Vec<BlockPos> = Vec::new();
    let mut max_range = 1;
    for &(ty, pos) in pois {
        max_range = max_range.max(ty.valid_range());
        if !targets.contains(&pos) {
            targets.push(pos);
        }
    }
    level.get_navigation(body, |navigation, level| navigation.create_path_to_targets(level, body, &targets, max_range))
}

struct JitteredLinearRetry {
    previous_attempt_timestamp: i64,
    next_scheduled_attempt_timestamp: i64,
    current_delay: i32,
}

const MIN_INTERVAL_INCREASE: i32 = 40;
const MAX_INTERVAL_INCREASE: i32 = 80;
const MAX_RETRY_PATHFINDING_INTERVAL: i32 = 400;

impl JitteredLinearRetry {
    fn new(random: &mut impl RandomSource, first_attempt_timestamp: i64) -> JitteredLinearRetry {
        let mut retry = JitteredLinearRetry { previous_attempt_timestamp: 0, next_scheduled_attempt_timestamp: 0, current_delay: 0 };
        retry.mark_attempt(random, first_attempt_timestamp);
        retry
    }

    fn mark_attempt(&mut self, random: &mut impl RandomSource, timestamp: i64) {
        self.previous_attempt_timestamp = timestamp;
        let suggested_delay = self.current_delay + random.next_int_bounded(MAX_INTERVAL_INCREASE - MIN_INTERVAL_INCREASE) + MIN_INTERVAL_INCREASE;
        self.current_delay = suggested_delay.min(MAX_RETRY_PATHFINDING_INTERVAL);
        self.next_scheduled_attempt_timestamp = timestamp + self.current_delay as i64;
    }

    fn is_still_valid(&self, timestamp: i64) -> bool {
        timestamp - self.previous_attempt_timestamp < MAX_RETRY_PATHFINDING_INTERVAL as i64
    }

    fn should_retry(&self, timestamp: i64) -> bool {
        timestamp >= self.next_scheduled_attempt_timestamp
    }
}
