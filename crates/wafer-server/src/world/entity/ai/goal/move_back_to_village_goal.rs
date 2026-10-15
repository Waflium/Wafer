use wafer_core::{BlockPos, Vec3};

use super::random_stroll_goal::RandomStrollGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::behavior_utils;
use crate::world::entity::ai::util::default_random_pos;

const MAX_XZ_DIST: i32 = 10;
const MAX_Y_DIST: i32 = 7;

pub struct MoveBackToVillageGoal {
    pub random_stroll_goal: RandomStrollGoal,
}

impl MoveBackToVillageGoal {
    pub fn new(speed_modifier: f64, check_no_action_time: bool) -> MoveBackToVillageGoal {
        MoveBackToVillageGoal { random_stroll_goal: RandomStrollGoal::with(speed_modifier, 10, check_no_action_time) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let pos = level.entity(mob).block_position();
        if level.is_village(pos) { false } else { self.random_stroll_goal.can_use(level, mob, get_position) }
    }
}

fn get_position(level: &mut ServerLevel, mob: EntityId) -> Option<Vec3> {
    let pos = level.entity(mob).block_position();
    let section_pos = BlockPos::new(pos.x >> 4, pos.y >> 4, pos.z >> 4);
    let optimal_section_pos = behavior_utils::find_section_closest_to_village(level, section_pos, 2);
    if optimal_section_pos != section_pos {
        let center = BlockPos::new((optimal_section_pos.x << 4) + 8, (optimal_section_pos.y << 4) + 8, (optimal_section_pos.z << 4) + 8);
        default_random_pos::get_pos_towards(level, mob, MAX_XZ_DIST, MAX_Y_DIST, Vec3::at_bottom_center_of(center), std::f32::consts::FRAC_PI_2 as f64)
    } else {
        None
    }
}
