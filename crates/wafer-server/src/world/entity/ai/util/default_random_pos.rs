use wafer_core::{BlockPos, Vec3};

use super::{goal_utils, random_pos};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub fn get_pos(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, vertical_dist: i32) -> Option<Vec3> {
    let restrict = goal_utils::mob_restricted(level, mob, horizontal_dist as f64);
    random_pos::generate_random_pos(level, mob, |level| {
        let direction = random_pos::generate_random_direction(&mut level.entity_mut(mob).random, horizontal_dist, vertical_dist);
        generate_random_pos_toward_direction(level, mob, horizontal_dist, restrict, direction)
    })
}

pub fn get_pos_away(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, vertical_dist: i32, avoid_pos: Vec3) -> Option<Vec3> {
    let dir_away = level.entity(mob).position().subtract(avoid_pos);
    let restrict = goal_utils::mob_restricted(level, mob, horizontal_dist as f64);
    random_pos::generate_random_pos(level, mob, |level| {
        let direction = random_pos::generate_random_direction_within_radians(
            &mut level.entity_mut(mob).random,
            0.0,
            horizontal_dist as f64,
            vertical_dist,
            0,
            dir_away.x,
            dir_away.z,
            std::f32::consts::FRAC_PI_2 as f64,
        )?;
        generate_random_pos_toward_direction(level, mob, horizontal_dist, restrict, direction)
    })
}

fn generate_random_pos_toward_direction(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, restrict: bool, direction: BlockPos) -> Option<BlockPos> {
    let pos = random_pos::generate_random_pos_toward_direction(level, mob, horizontal_dist as f64, direction);
    let valid = !goal_utils::is_outside_limits(level, pos)
        && !goal_utils::is_restricted(level, restrict, mob, pos)
        && !goal_utils::is_not_stable(level, mob, pos)
        && !goal_utils::has_malus(level, mob, pos);
    valid.then_some(pos)
}
