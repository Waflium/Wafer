use wafer_core::Vec3;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::memory_value::EntityValue;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::land_random_pos;

pub fn entity(memory: MemoryModuleType<EntityValue>, speed_modifier: f32, desired_distance: i32, interrupt_current_walk: bool) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let walk_away_from = i.present(memory);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = walk_target.try_get(level, body);
            if target.is_some() && !interrupt_current_walk {
                return false;
            }
            let body_position = level.entity(body).position();
            let avoid_entity = walk_away_from.get(level, body);
            let Some(avoid_position) = level.entity_ref(avoid_entity).map(|e| e.position()) else { return false };
            if !closer_than(body_position, avoid_position, desired_distance as f64) {
                return false;
            }
            if let Some(target) = &target
                && target.get_speed_modifier() == speed_modifier
            {
                let current_direction = target.get_target().current_position(level).subtract(body_position);
                let avoid_direction = avoid_position.subtract(body_position);
                if current_direction.dot(avoid_direction) < 0.0 {
                    return false;
                }
            }
            for _ in 0..10 {
                if let Some(flee_to_pos) = land_random_pos::get_pos_away(level, body, 16, 7, avoid_position) {
                    walk_target.set(level, body, WalkTarget::of_vec(flee_to_pos, speed_modifier, 0));
                    break;
                }
            }
            true
        }
    })
}

fn closer_than(a: Vec3, b: Vec3, distance: f64) -> bool {
    a.distance_to_sqr(b) < distance * distance
}
