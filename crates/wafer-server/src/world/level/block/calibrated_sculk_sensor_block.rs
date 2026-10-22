use wafer_core::{BlockPos, Direction};
use wafer_data::properties::HORIZONTAL_FACING;
use wafer_data::{Block, BlockState};

use super::BlockBehaviour;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::sculk_sensor_block::SculkSensorBlock;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct CalibratedSculkSensorBlock;

impl BlockBehaviour for CalibratedSculkSensorBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        SculkSensorBlock.get_state_for_placement(block, ctx, level).map(|state| state.set(HORIZONTAL_FACING, ctx.horizontal_direction()))
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        SculkSensorBlock.tick(state, level, pos);
    }

    fn step_on(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, entity: EntityId) {
        SculkSensorBlock.step_on(level, pos, state, entity);
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, moved_by_piston: bool) {
        SculkSensorBlock.on_place(state, level, pos, old_state, moved_by_piston);
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, moved_by_piston: bool) {
        SculkSensorBlock.affect_neighbors_after_removal(state, level, pos, moved_by_piston);
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, direction: Direction, neighbour_pos: BlockPos, neighbour_state: BlockState) -> BlockState {
        SculkSensorBlock.update_shape(state, level, pos, direction, neighbour_pos, neighbour_state)
    }

    fn get_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        if direction != state.get(HORIZONTAL_FACING) { SculkSensorBlock.own_signal(state, level, pos) } else { 0 }
    }

    fn own_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> i32 {
        SculkSensorBlock.own_signal(state, level, pos)
    }

    fn get_direct_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos, direction: Direction) -> i32 {
        if direction == Direction::Up { self.get_signal(state, level, pos, direction) } else { 0 }
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, direction: Direction) -> i32 {
        SculkSensorBlock.get_analog_output_signal(state, level, pos, direction)
    }

    fn spawn_after_break(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, drop_experience: bool) {
        SculkSensorBlock.spawn_after_break(state, level, pos, tool, drop_experience);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        state.set(HORIZONTAL_FACING, rotation.rotate(state.get(HORIZONTAL_FACING)))
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        super::rotate(state, mirror.get_rotation(state.get(HORIZONTAL_FACING)))
    }
}
