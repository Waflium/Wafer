use wafer_core::BlockPos;
use wafer_data::properties::{BED_PART, BedPart, LIT};
use wafer_data::{BlockState, block_tags, blocks};

use super::move_to_block_goal::MoveToBlockGoal;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::block::entity::chest_block_entity::ChestBlockEntity;
use crate::world::level::path_navigation_region::PathNavigationRegion;

type BlockValidation = (fn(BlockState) -> bool, fn(BlockState, &ServerLevel, BlockPos) -> bool);

const BLOCK_TYPE_VALIDATION: [BlockValidation; 3] = [
    (|block_state| block_state.is_block(blocks::CHEST), |_, level, pos| ChestBlockEntity::get_open_count(level, pos) < 1),
    (|block_state| block_state.is_block(blocks::FURNACE), |block_state, _, _| block_state.get(LIT)),
    (|block_state| block_state.is(&block_tags::BEDS), |block_state, _, _| block_state.try_get(BED_PART).is_none_or(|part| part != BedPart::Head)),
];

pub struct CatSitOnBlockGoal {
    pub move_to_block_goal: MoveToBlockGoal,
}

impl CatSitOnBlockGoal {
    pub fn new(speed_modifier: f64) -> CatSitOnBlockGoal {
        CatSitOnBlockGoal { move_to_block_goal: MoveToBlockGoal::new(speed_modifier, 8, 1) }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.is_tame(mob) && !level.is_ordered_to_sit(mob) && self.move_to_block_goal.can_use(level, mob, &is_valid_target)
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel) -> bool {
        self.move_to_block_goal.can_continue_to_use(level, &is_valid_target)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.start(level, mob);
        level.set_in_sitting_pose(mob, false);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.set_in_sitting_pose(mob, false);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.move_to_block_goal.tick(level, mob, 1.0);
        level.set_in_sitting_pose(mob, self.move_to_block_goal.is_reached_target());
    }
}

fn is_valid_target(level: &ServerLevel, pos: BlockPos) -> bool {
    let region = PathNavigationRegion::new(level);
    if !region.get_block_state(pos.above()).is_air() {
        return false;
    }
    let block_state = region.get_block_state(pos);
    if !block_state.is(&block_tags::CATS_CAN_SIT_ON) {
        return false;
    }
    for (applies, validate) in BLOCK_TYPE_VALIDATION {
        if applies(block_state) {
            return validate(block_state, level, pos);
        }
    }
    true
}
