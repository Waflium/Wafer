use wafer_core::{BlockPos, Vec3};

use super::{goal_utils, random_pos};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub fn get_pos(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, vertical_dist: i32) -> Option<Vec3> {
    let restrict = goal_utils::mob_restricted(level, mob, horizontal_dist as f64);
    random_pos::generate_random_pos(level, mob, |level| {
        let direction = random_pos::generate_random_direction(&mut level.entity_mut(mob).random, horizontal_dist, vertical_dist);
        let pos = generate_random_pos_toward_direction(level, mob, horizontal_dist as f64, restrict, direction)?;
        move_pos_up_out_of_solid(level, mob, pos)
    })
}

pub fn get_pos_weighted(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, vertical_dist: i32, position_weight: impl FnMut(&mut ServerLevel, BlockPos) -> f64) -> Option<Vec3> {
    let restrict = goal_utils::mob_restricted(level, mob, horizontal_dist as f64);
    random_pos::generate_random_pos_weighted(
        level,
        |level| {
            let direction = random_pos::generate_random_direction(&mut level.entity_mut(mob).random, horizontal_dist, vertical_dist);
            let pos = generate_random_pos_toward_direction(level, mob, horizontal_dist as f64, restrict, direction)?;
            move_pos_up_out_of_solid(level, mob, pos)
        },
        position_weight,
    )
}

pub fn get_pos_towards(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, vertical_dist: i32, towards_pos: Vec3) -> Option<Vec3> {
    let e = level.entity(mob);
    let dir = towards_pos.subtract(Vec3::new(e.x(), e.y(), e.z()));
    let restrict = goal_utils::mob_restricted(level, mob, horizontal_dist as f64);
    get_pos_in_direction(level, mob, 0.0, horizontal_dist as f64, vertical_dist, dir, restrict)
}

pub fn get_pos_away(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, vertical_dist: i32, avoid_pos: Vec3) -> Option<Vec3> {
    get_pos_away_between(level, mob, 0.0, horizontal_dist as f64, vertical_dist, avoid_pos)
}

pub fn get_pos_away_between(level: &mut ServerLevel, mob: EntityId, min_horizontal_dist: f64, max_horizontal_dist: f64, vertical_dist: i32, avoid_pos: Vec3) -> Option<Vec3> {
    let mut dir_away = level.entity(mob).position().subtract(avoid_pos);
    if dir_away.length() == 0.0 {
        let random = &mut level.entity_mut(mob).random;
        let x = wafer_core::random::RandomSource::next_double(random) - 0.5;
        let z = wafer_core::random::RandomSource::next_double(random) - 0.5;
        dir_away = Vec3::new(x, 0.0, z);
    }
    let restrict = goal_utils::mob_restricted(level, mob, max_horizontal_dist);
    get_pos_in_direction(level, mob, min_horizontal_dist, max_horizontal_dist, vertical_dist, dir_away, restrict)
}

fn get_pos_in_direction(level: &mut ServerLevel, mob: EntityId, min_horizontal_dist: f64, max_horizontal_dist: f64, vertical_dist: i32, dir: Vec3, restrict: bool) -> Option<Vec3> {
    random_pos::generate_random_pos(level, mob, |level| {
        let random = &mut level.entity_mut(mob).random;
        let direction = random_pos::generate_random_direction_within_radians(
            random,
            min_horizontal_dist,
            max_horizontal_dist,
            vertical_dist,
            0,
            dir.x,
            dir.z,
            std::f32::consts::FRAC_PI_2 as f64,
        )?;
        let pos = generate_random_pos_toward_direction(level, mob, max_horizontal_dist, restrict, direction)?;
        move_pos_up_out_of_solid(level, mob, pos)
    })
}

pub fn move_pos_up_out_of_solid(level: &ServerLevel, mob: EntityId, pos: BlockPos) -> Option<BlockPos> {
    let pos = random_pos::move_up_out_of_solid(pos, level.chunks.max_y(), |block_pos| goal_utils::is_solid(level, block_pos));
    (!goal_utils::is_water(level, pos) && !goal_utils::has_malus(level, mob, pos)).then_some(pos)
}

pub fn generate_random_pos_toward_direction(level: &mut ServerLevel, mob: EntityId, horizontal_dist: f64, restrict: bool, direction: BlockPos) -> Option<BlockPos> {
    let pos = random_pos::generate_random_pos_toward_direction(level, mob, horizontal_dist, direction);
    let valid = !goal_utils::is_outside_limits(level, pos) && !goal_utils::is_restricted(level, restrict, mob, pos) && !goal_utils::is_not_stable(level, mob, pos);
    valid.then_some(pos)
}
