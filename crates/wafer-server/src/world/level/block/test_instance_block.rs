use wafer_core::BlockPos;
use wafer_data::BlockState;

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;

pub struct TestInstanceBlock;

impl BlockBehaviour for TestInstanceBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if level.test_instance_block_entity(pos).is_none() {
            return InteractionResult::Pass;
        }
        if !level.can_use_game_master_blocks(player) {
            return InteractionResult::Pass;
        }
        InteractionResult::SUCCESS
    }
}
