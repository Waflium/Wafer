use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::land_random_pos;

const MIN_TIME_BETWEEN_STROLLS: i64 = 180;
const STROLL_MAX_XZ_DIST: i32 = 8;
const STROLL_MAX_Y_DIST: i32 = 6;

pub fn create(memory_type: MemoryModuleType<GlobalPos>, speed_modifier: f32, max_distance_from_poi: i32) -> OneShot {
    let mut next_ok_start_time: i64 = 0;
    behavior_builder::create(|i| {
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let memory = i.present(memory_type);
        move |level: &mut ServerLevel, body: EntityId, timestamp: i64| {
            let pos = memory.get(level, body);
            if level.dimension != pos.dimension || !pos.pos.closer_to_center_than(level.entity(body).position(), max_distance_from_poi as f64) {
                return false;
            }
            if timestamp <= next_ok_start_time {
                return true;
            }
            let land_pos = land_random_pos::get_pos(level, body, STROLL_MAX_XZ_DIST, STROLL_MAX_Y_DIST);
            walk_target.set_or_erase(level, body, land_pos.map(|p| WalkTarget::of_vec(p, speed_modifier, 1)));
            next_ok_start_time = timestamp + MIN_TIME_BETWEEN_STROLLS;
            true
        }
    })
}
