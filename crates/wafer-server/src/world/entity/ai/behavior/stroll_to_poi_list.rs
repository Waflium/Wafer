use wafer_core::random::RandomSource;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub fn create(
    stroll_to_memory_type: MemoryModuleType<Vec<GlobalPos>>,
    speed_modifier: f32,
    close_enough_dist: i32,
    max_distance_from_poi: i32,
    must_be_close_to_memory_type: MemoryModuleType<GlobalPos>,
) -> OneShot {
    let mut next_ok_start_time: i64 = 0;
    behavior_builder::create(|i| {
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let stroll_to_memory = i.present(stroll_to_memory_type);
        let must_be_close_to_memory = i.present(must_be_close_to_memory_type);
        move |level: &mut ServerLevel, body: EntityId, timestamp: i64| {
            let stroll_to = stroll_to_memory.get(level, body);
            let stay_close_to = must_be_close_to_memory.get(level, body);
            if stroll_to.is_empty() {
                return false;
            }
            let target_pos = stroll_to[level.random.next_int_bounded(stroll_to.len() as i32) as usize].clone();
            if level.dimension == target_pos.dimension && stay_close_to.pos.closer_to_center_than(level.entity(body).position(), max_distance_from_poi as f64) {
                if timestamp > next_ok_start_time {
                    walk_target.set(level, body, WalkTarget::of_block_pos(target_pos.pos, speed_modifier, close_enough_dist));
                    next_ok_start_time = timestamp + 100;
                }
                true
            } else {
                false
            }
        }
    })
}
