use wafer_core::BlockPos;
use wafer_core::random::RandomSource;

use super::behavior_utils;
use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};

fn get_nearby_pos(level: &mut ServerLevel, pos: BlockPos) -> BlockPos {
    let x = get_random_offset(level);
    let z = get_random_offset(level);
    pos.offset(x, 0, z)
}

fn get_random_offset(level: &mut ServerLevel) -> i32 {
    level.random.next_int_bounded(3) - 1
}

pub fn create(location_memory: MemoryModuleType<BlockPos>, close_enough_dist: i32, speed_modifier: f32) -> OneShot {
    behavior_builder::create(|i| {
        let location = i.present(location_memory);
        let _attack_target = i.absent(memory_module_type::ATTACK_TARGET);
        let _walk_target = i.absent(memory_module_type::WALK_TARGET);
        let _look_target = i.registered(memory_module_type::LOOK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let celebrate_location = location.get(level, body);
            let close_enough_to_target = celebrate_location.closer_than(level.entity(body).block_position(), close_enough_dist as f64);
            if !close_enough_to_target {
                let target = get_nearby_pos(level, celebrate_location);
                behavior_utils::set_walk_and_look_target_memories_to_pos(level, body, target, speed_modifier, close_enough_dist);
            }
            true
        }
    })
}
