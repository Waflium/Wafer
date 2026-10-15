use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::core::global_pos::GlobalPos;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub fn create(memory_type: MemoryModuleType<GlobalPos>, speed_modifier: f32, close_enough_dist: i32, max_distance_from_poi: i32) -> OneShot {
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
            walk_target.set(level, body, WalkTarget::of_block_pos(pos.pos, speed_modifier, close_enough_dist));
            next_ok_start_time = timestamp + 80;
            true
        }
    })
}
