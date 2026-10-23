use wafer_core::BlockPos;
use wafer_core::shapes::{self, BooleanOp, VoxelShape};
use wafer_data::properties::{EXTENDED, FACING};
use wafer_data::BlockState;

use super::entity::BlockEntityKind;
use super::{BlockBehaviour, BlockHitResult};
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::storage::loot::loot_params::LootParams;

pub struct MovingPistonBlock;

impl BlockBehaviour for MovingPistonBlock {
    fn destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState) {
        let relative = pos.relative(state.get(FACING).opposite());
        let block_state = level.block_state(relative);
        if block_state.block().is_instance_of("PistonBaseBlock") && block_state.get(EXTENDED) {
            level.remove_block(relative, false);
        }
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if level.get_block_entity(pos).is_none() {
            level.remove_block(pos, false);
            InteractionResult::CONSUME
        } else {
            InteractionResult::Pass
        }
    }

    fn get_drops(&self, _state: BlockState, params: LootParams) -> Vec<ItemStack> {
        let Some(origin) = params.origin else { return Vec::new() };
        let pos = BlockPos::containing(origin.x, origin.y, origin.z);
        let Some(moved_state) = params.level.piston_moving_block_entity(pos).map(|entity| entity.get_moved_state()) else { return Vec::new() };
        super::of(moved_state).get_drops(moved_state, params)
    }

    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, _state: BlockState, _include_data: bool) -> ItemStack {
        ItemStack::empty()
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }
}

pub fn get_collision_shape(level: &ServerLevel, pos: BlockPos) -> VoxelShape {
    let Some(BlockEntityKind::PistonMoving(entity)) = level.chunks.loaded_chunk(pos.chunk()).and_then(|chunk| chunk.block_entities.map.get(&pos)).map(|entity| &entity.kind) else {
        return VoxelShape::empty();
    };
    let mut shape = VoxelShape::empty();
    for (part, offset) in entity.collision_shapes(pos).into_iter().flatten() {
        shape = shapes::join_unoptimized(&shape, &part.moved(offset[0], offset[1], offset[2]), BooleanOp::Or);
    }
    shape
}
