use wafer_core::Vec3;
use wafer_core::random::RandomSource;

use super::{goal_utils, land_random_pos, random_pos};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[allow(clippy::too_many_arguments)]
pub fn get_pos(
    level: &mut ServerLevel,
    mob: EntityId,
    horizontal_dist: i32,
    vertical_dist: i32,
    x_dir: f64,
    z_dir: f64,
    max_xz_radians_difference: f32,
    hover_max_height: i32,
    hover_min_height: i32,
) -> Option<Vec3> {
    let restrict = goal_utils::mob_restricted(level, mob, horizontal_dist as f64);
    random_pos::generate_random_pos(level, mob, |level| {
        let direction = random_pos::generate_random_direction_within_radians(
            &mut level.entity_mut(mob).random,
            0.0,
            horizontal_dist as f64,
            vertical_dist,
            0,
            x_dir,
            z_dir,
            max_xz_radians_difference as f64,
        )?;
        let pos = land_random_pos::generate_random_pos_toward_direction(level, mob, horizontal_dist as f64, restrict, direction)?;
        let above_solid_amount = level.entity_mut(mob).random.next_int_bounded(hover_max_height - hover_min_height + 1) + hover_min_height;
        let max_y = level.chunks.max_y();
        let pos = random_pos::move_up_to_above_solid(pos, above_solid_amount, max_y, |block_pos| goal_utils::is_solid(level, block_pos));
        (!goal_utils::is_water(level, pos) && !goal_utils::has_malus(level, mob, pos)).then_some(pos)
    })
}
