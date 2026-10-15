use wafer_core::BlockPos;
use wafer_data::block_tags;

use super::move_to_block_goal::MoveToBlockGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::path_navigation_region::PathNavigationRegion;

pub struct CatLieOnBlockGoal {
    pub move_to_block_goal: MoveToBlockGoal,
}

impl CatLieOnBlockGoal {
    pub fn new(speed_modifier: f64, search_range: i32) -> CatLieOnBlockGoal {
        let mut move_to_block_goal = MoveToBlockGoal::new(speed_modifier, search_range, 6);
        move_to_block_goal.vertical_search_start = -2;
        CatLieOnBlockGoal { move_to_block_goal }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.is_tame(mob)
            && !level.is_ordered_to_sit(mob)
            && !level.is_lying(mob)
            && self.move_to_block_goal.can_use_with(level, mob, |_, _| CatLieOnBlockGoal::next_start_tick(), &is_valid_target)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel) -> bool {
        self.move_to_block_goal.can_continue_to_use(level, &is_valid_target)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.start(level, mob);
        level.set_in_sitting_pose(mob, false);
    }

    fn next_start_tick() -> i32 {
        40
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_lying(mob, false);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.tick(level, mob, 1.0);
        level.set_in_sitting_pose(mob, false);
        if !self.move_to_block_goal.is_reached_target() {
            level.set_lying(mob, false);
        } else if !level.is_lying(mob) {
            level.set_lying(mob, true);
        }
    }
}

fn is_valid_target(level: &ServerLevel, pos: BlockPos) -> bool {
    let region = PathNavigationRegion::new(level);
    region.get_block_state(pos.above()).is_air() && region.get_block_state(pos).is(&block_tags::CATS_CAN_LIE_ON)
}
