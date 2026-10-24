use std::borrow::Cow;

use wafer_core::shapes::VoxelShape;
use wafer_core::{AABB, BlockPos};
use wafer_data::{BlockState, FluidState, blocks};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::{moving_piston_block, shulker_box_block};

pub struct PathNavigationRegion<'a> {
    pub level: &'a ServerLevel,
}

impl<'a> PathNavigationRegion<'a> {
    pub fn new(level: &'a ServerLevel) -> PathNavigationRegion<'a> {
        PathNavigationRegion { level }
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        if self.level.chunks.is_outside_build_height(pos.y) {
            return blocks::AIR.default_state();
        }
        self.level.chunks.loaded_chunk(pos.chunk()).map_or(blocks::VOID_AIR.default_state(), |chunk| chunk.block_state(pos))
    }

    pub fn get_fluid_state(&self, pos: BlockPos) -> FluidState {
        self.get_block_state(pos).fluid_state()
    }

    pub fn get_state_collision_shape(&self, state: BlockState, pos: BlockPos) -> Cow<'static, VoxelShape> {
        if state.is_block(blocks::MOVING_PISTON) {
            Cow::Owned(moving_piston_block::get_collision_shape(self.level, pos))
        } else if state.has_dynamic_shape() && state.block().is_instance_of("ShulkerBoxBlock") {
            let shulker = match self.level.chunks.block_entity_ref(pos).map(|entity| &entity.kind) {
                Some(BlockEntityKind::ShulkerBox(shulker)) => Some(shulker),
                _ => None,
            };
            Cow::Borrowed(shulker_box_block::shape_of(shulker, state))
        } else {
            state.get_collision_shape(pos)
        }
    }

    pub fn no_collision(&self, entity: EntityId, aabb: &AABB) -> bool {
        self.level.no_collision_for(entity, aabb)
    }

    pub fn get_min_y(&self) -> i32 {
        self.level.chunks.min_y()
    }
}
