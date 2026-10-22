use wafer_core::{BlockPos, Vec3};

use super::{goal_utils, random_pos};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[allow(clippy::too_many_arguments)]
pub fn get_pos(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, vertical_dist: i32, flying_height: i32, x_dir: f64, z_dir: f64, max_xz_radians_difference: f64) -> Option<Vec3> {
    let restrict = goal_utils::mob_restricted(level, mob, horizontal_dist as f64);
    random_pos::generate_random_pos(level, mob, |level| generate_random_pos(level, mob, horizontal_dist, vertical_dist, flying_height, x_dir, z_dir, max_xz_radians_difference, restrict))
}

#[allow(clippy::too_many_arguments)]
pub fn generate_random_pos(
    level: &mut ServerLevel,
    mob: EntityId,
    horizontal_dist: i32,
    vertical_dist: i32,
    flying_height: i32,
    x_dir: f64,
    z_dir: f64,
    max_xz_radians_difference: f64,
    restrict: bool,
) -> Option<BlockPos> {
    let direction = random_pos::generate_random_direction_within_radians(
        &mut level.entity_mut(mob).random,
        0.0,
        horizontal_dist as f64,
        vertical_dist,
        flying_height,
        x_dir,
        z_dir,
        max_xz_radians_difference,
    )?;
    let pos = random_pos::generate_random_pos_toward_direction(level, mob, horizontal_dist as f64, direction);
    if goal_utils::is_outside_limits(level, pos) || goal_utils::is_restricted(level, restrict, mob, pos) {
        return None;
    }
    let max_y = level.chunks.max_y();
    let pos = random_pos::move_up_out_of_solid(pos, max_y, |block_pos| goal_utils::is_solid(level, block_pos));
    (!goal_utils::has_malus(level, mob, pos)).then_some(pos)
}
