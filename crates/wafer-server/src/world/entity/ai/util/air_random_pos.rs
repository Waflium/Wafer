use wafer_core::Vec3;

use super::{air_and_water_random_pos, goal_utils, random_pos};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub fn get_pos_towards(level: &mut ServerLevel, mob: EntityId, horizontal_dist: i32, vertical_dist: i32, flying_height: i32, towards_pos: Vec3, max_xz_radians_from_dir: f64) -> Option<Vec3> {
    let e = level.entity(mob);
    let dir = towards_pos.subtract(Vec3::new(e.x(), e.y(), e.z()));
    let restrict = goal_utils::mob_restricted(level, mob, horizontal_dist as f64);
    random_pos::generate_random_pos(level, mob, |level| {
        let pos = air_and_water_random_pos::generate_random_pos(level, mob, horizontal_dist, vertical_dist, flying_height, dir.x, dir.z, max_xz_radians_from_dir, restrict);
        pos.filter(|&pos| !goal_utils::is_water(level, pos))
    })
}
