use wafer_core::Vec3;

use super::water_avoiding_random_stroll_goal::WaterAvoidingRandomStrollGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::util::{air_and_water_random_pos, hover_random_pos};
use crate::world::entity::{EntityId, calculate_view_vector};

pub struct WaterAvoidingRandomFlyingGoal {
    pub water_avoiding_random_stroll_goal: WaterAvoidingRandomStrollGoal,
}

impl WaterAvoidingRandomFlyingGoal {
    pub fn new(speed_modifier: f64) -> WaterAvoidingRandomFlyingGoal {
        WaterAvoidingRandomFlyingGoal { water_avoiding_random_stroll_goal: WaterAvoidingRandomStrollGoal::new(speed_modifier) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.water_avoiding_random_stroll_goal.random_stroll_goal.can_use(level, mob, get_position)
    }
}

pub fn get_position(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    let e = level.entity(mob);
    let wander_direction = calculate_view_vector(e.x_rot_o, level.living(mob).y_head_rot_o);
    let max_xz_radians = std::f32::consts::FRAC_PI_2;
    hover_random_pos::get_pos(level, mob, 8, 7, wander_direction.x, wander_direction.z, max_xz_radians, 3, 1)
        .or_else(|| air_and_water_random_pos::get_pos(level, mob, 8, 4, -2, wander_direction.x, wander_direction.z, max_xz_radians as f64))
}
