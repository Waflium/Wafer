use wafer_core::{BlockPos, Vec3};

use super::behavior_utils;
use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::{goal_utils, land_random_pos};
use crate::world::level::BlockGetter;

const MAX_XZ_DIST: i32 = 10;
const MAX_Y_DIST: i32 = 7;
const SWIM_XY_DISTANCE_TIERS: [[i32; 2]; 6] = [[1, 1], [3, 3], [5, 5], [6, 5], [7, 7], [10, 7]];

pub fn stroll(speed_modifier: f32) -> OneShot {
    stroll_from_water(speed_modifier, true)
}

pub fn stroll_from_water(speed_modifier: f32, may_stroll_from_water: bool) -> OneShot {
    stroll_fly_or_swim(
        speed_modifier,
        |level, body| land_random_pos::get_pos(level, body, MAX_XZ_DIST, MAX_Y_DIST),
        move |level, body| may_stroll_from_water || !level.entity(body).is_in_water(),
    )
}

pub fn swim(speed_modifier: f32) -> OneShot {
    stroll_fly_or_swim(speed_modifier, get_target_swim_pos, |level, body| level.entity(body).is_in_water())
}

fn get_target_swim_pos(level: &mut ServerLevel, body: EntityId) -> Option<Vec3> {
    let mut fallback: Option<Vec3> = None;
    let mut target_pos: Option<Vec3> = None;
    for [horizontal, vertical] in SWIM_XY_DISTANCE_TIERS {
        target_pos = match fallback {
            None => behavior_utils::get_random_swimmable_pos(level, body, horizontal, vertical),
            Some(fallback) => {
                let position = level.entity(body).position();
                Some(position.add_vec(position.vector_to(fallback).normalize().multiply(horizontal as f64, vertical as f64, horizontal as f64)))
            }
        };
        let restrict = goal_utils::mob_restricted(level, body, horizontal as f64);
        match target_pos {
            Some(pos) if !level.fluid_state(BlockPos::containing_vec(pos)).is_empty() && !goal_utils::is_restricted(level, restrict, body, BlockPos::containing_vec(pos)) => fallback = Some(pos),
            _ => return fallback,
        }
    }
    target_pos
}

fn stroll_fly_or_swim(
    speed_modifier: f32,
    fetch_target_pos: impl Fn(&mut ServerLevel, EntityId) -> Option<Vec3> + 'static,
    can_run: impl Fn(&ServerLevel, EntityId) -> bool + 'static,
) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if !can_run(level, body) {
                return false;
            }
            let path_goal_pos = fetch_target_pos(level, body);
            walk_target.set_or_erase(level, body, path_goal_pos.map(|pos| WalkTarget::of_vec(pos, speed_modifier, 0)));
            true
        }
    })
}
