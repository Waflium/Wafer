use wafer_core::{BlockPos, Vec3};

use super::behavior_utils;
use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::ai::util::{default_random_pos, land_random_pos};

const MAX_XZ_DIST: i32 = 10;
const MAX_Y_DIST: i32 = 7;

pub fn create(speed_modifier: f32) -> OneShot {
    create_with(speed_modifier, MAX_XZ_DIST, MAX_Y_DIST)
}

pub fn create_with(speed_modifier: f32, max_xy_dist: i32, max_y_dist: i32) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let body_pos = level.entity(body).block_position();
            let land_pos = if level.is_village(body_pos) {
                land_random_pos::get_pos(level, body, max_xy_dist, max_y_dist)
            } else {
                let section_pos = BlockPos::new(body_pos.x >> 4, body_pos.y >> 4, body_pos.z >> 4);
                let optimal_section_pos = behavior_utils::find_section_closest_to_village(level, section_pos, 2);
                if optimal_section_pos != section_pos {
                    let center = BlockPos::new((optimal_section_pos.x << 4) + 8, (optimal_section_pos.y << 4) + 8, (optimal_section_pos.z << 4) + 8);
                    default_random_pos::get_pos_towards(level, body, max_xy_dist, max_y_dist, Vec3::at_bottom_center_of(center), std::f32::consts::FRAC_PI_2 as f64)
                } else {
                    land_random_pos::get_pos(level, body, max_xy_dist, max_y_dist)
                }
            };
            walk_target.set_or_erase(level, body, land_pos.map(|pos| WalkTarget::of_vec(pos, speed_modifier, 0)));
            true
        }
    })
}
