use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{EXTENDED, FACING, PISTON_TYPE, PistonType};
use wafer_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::redstone::orientation::Orientation;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct PistonHeadBlock;

fn is_fitting_base(arm_state: BlockState, potential_base: BlockState) -> bool {
    let base_block = if arm_state.get(PISTON_TYPE) == PistonType::Default { blocks::PISTON } else { blocks::STICKY_PISTON };
    potential_base.is_block(base_block) && potential_base.get(EXTENDED) && potential_base.get(FACING) == arm_state.get(FACING)
}

impl BlockBehaviour for PistonHeadBlock {
    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        if level.prevents_block_drops(player) {
            let base_pos = pos.relative(state.get(FACING).opposite());
            if is_fitting_base(state, crate::world::level::BlockGetter::block_state(level, base_pos)) {
                level.destroy_block(base_pos, false, None, crate::world::level::update_flags::UPDATE_LIMIT);
            }
        }
        super::block_player_will_destroy(level, pos, state, player)
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        let base_pos = pos.relative(state.get(FACING).opposite());
        if is_fitting_base(state, crate::world::level::BlockGetter::block_state(level, base_pos)) {
            level.destroy_block(base_pos, true, None, crate::world::level::update_flags::UPDATE_LIMIT);
        }
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        _neighbour_pos: BlockPos,
        _neighbour_state: BlockState,
    ) -> BlockState {
        if direction.opposite() == state.get(FACING) && !self.can_survive(state, level, pos) { blocks::AIR.default_state() } else { state }
    }

    fn can_survive(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        let base = level.block_state(pos.relative(state.get(FACING).opposite()));
        is_fitting_base(state, base) || base.is_block(blocks::MOVING_PISTON) && base.get(FACING) == state.get(FACING)
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, orientation: Option<Orientation>, _moved_by_piston: bool) {
        if self.can_survive(state, level, pos) {
            level.neighbor_changed(pos.relative(state.get(FACING).opposite()), block, crate::world::level::redstone::experimental_redstone_utils::with_front(orientation, state.get(FACING).opposite()));
        }
    }

    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
        let base = if state.get(PISTON_TYPE) == PistonType::Sticky { blocks::STICKY_PISTON } else { blocks::PISTON };
        ItemStack::new(base.item(), 1)
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(FACING, rotation.rotate(state.get(FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(FACING)))
    }
}
