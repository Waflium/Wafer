use wafer_core::BlockPos;
use wafer_data::BlockState;
use wafer_data::properties::ORIENTATION;

use super::entity::BlockEntityKind;
use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use super::mirror::Mirror;
use super::rotation::Rotation;

pub struct JigsawBlock;

impl BlockBehaviour for JigsawBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if matches!(level.get_block_entity(pos).map(|entity| &entity.kind), Some(BlockEntityKind::Jigsaw(_))) && level.can_use_game_master_blocks(player) {
            InteractionResult::SUCCESS
        } else {
            InteractionResult::Pass
        }
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(ORIENTATION, state.get(ORIENTATION).rotate(rotation.rotation()))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        state.set(ORIENTATION, state.get(ORIENTATION).rotate(mirror.rotation()))
    }
}
