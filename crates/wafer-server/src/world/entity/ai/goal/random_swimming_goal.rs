use wafer_core::Vec3;

use super::random_stroll_goal::RandomStrollGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::behavior_utils;

pub struct RandomSwimmingGoal {
    pub random_stroll_goal: RandomStrollGoal,
}

impl RandomSwimmingGoal {
    pub fn new(speed_modifier: f64, interval: i32) -> RandomSwimmingGoal {
        RandomSwimmingGoal { random_stroll_goal: RandomStrollGoal::with_interval(speed_modifier, interval) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.random_stroll_goal.can_use(level, mob, RandomSwimmingGoal::get_position)
    }

    pub fn get_position(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
        behavior_utils::get_random_swimmable_pos(level, mob, 10, 7)
    }
}
