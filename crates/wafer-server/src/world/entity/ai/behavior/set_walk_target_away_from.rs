use wafer_core::{BlockPos, Vec3};

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::memory_value::EntityValue;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::land_random_pos;

pub fn pos(memory: MemoryModuleType<BlockPos>, speed_modifier: f32, desired_distance: i32, interrupt_current_walk: bool) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let walk_away_from = i.present(memory);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = walk_target.try_get(level, body);
            if target.is_some() && !interrupt_current_walk {
                return false;
            }
            let avoid_position = Vec3::at_bottom_center_of(walk_away_from.get(level, body));
            match walk_away(level, body, target, avoid_position, speed_modifier, desired_distance) {
                Some(new_target) => {
                    if let Some(new_target) = new_target {
                        walk_target.set(level, body, new_target);
                    }
                    true
                }
                None => false,
            }
        }
    })
}

pub fn entity(memory: MemoryModuleType<EntityValue>, speed_modifier: f32, desired_distance: i32, interrupt_current_walk: bool) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let walk_away_from = i.present(memory);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = walk_target.try_get(level, body);
            if target.is_some() && !interrupt_current_walk {
                return false;
            }
            let avoid_entity = walk_away_from.get(level, body);
            let Some(avoid_position) = level.entity_ref(avoid_entity).map(|e| e.position()) else { return false };
            match walk_away(level, body, target, avoid_position, speed_modifier, desired_distance) {
                Some(new_target) => {
                    if let Some(new_target) = new_target {
                        walk_target.set(level, body, new_target);
                    }
                    true
                }
                None => false,
            }
        }
    })
}

fn walk_away(level: &mut ServerLevel, body: EntityId, target: Option<WalkTarget>, avoid_position: Vec3, speed_modifier: f32, desired_distance: i32) -> Option<Option<WalkTarget>> {
    let body_position = level.entity(body).position();
    if !closer_than(body_position, avoid_position, desired_distance as f64) {
        return None;
    }
    if let Some(target) = &target
        && target.get_speed_modifier() == speed_modifier
    {
        let current_direction = target.get_target().current_position(level).subtract(body_position);
        let avoid_direction = avoid_position.subtract(body_position);
        if current_direction.dot(avoid_direction) < 0.0 {
            return None;
        }
    }
    for _ in 0..10 {
        if let Some(flee_to_pos) = land_random_pos::get_pos_away(level, body, 16, 7, avoid_position) {
            return Some(Some(WalkTarget::of_vec(flee_to_pos, speed_modifier, 0)));
        }
    }
    Some(None)
}

fn closer_than(a: Vec3, b: Vec3, distance: f64) -> bool {
    a.distance_to_sqr(b) < distance * distance
}
