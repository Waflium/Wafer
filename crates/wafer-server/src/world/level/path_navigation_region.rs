use std::borrow::Cow;

use wafer_core::shapes::VoxelShape;
use wafer_core::{AABB, BlockPos, ChunkPos};
use wafer_data::{BlockState, FluidState, blocks};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::{moving_piston_block, shulker_box_block};
use crate::world::phys::shapes::collision_context::CollisionContext;

pub struct PathNavigationRegion<'a> {
    pub level: &'a ServerLevel,
    snapshot: bool,
}

impl<'a> PathNavigationRegion<'a> {
    pub fn new(level: &'a ServerLevel) -> PathNavigationRegion<'a> {
        PathNavigationRegion { level, snapshot: false }
    }

    pub fn snapshot(level: &'a ServerLevel, start: BlockPos, end: BlockPos) -> PathNavigationRegion<'a> {
        for chunk_x in (start.x >> 4)..=(end.x >> 4) {
            for chunk_z in (start.z >> 4)..=(end.z >> 4) {
                level.chunks.get_chunk_now(ChunkPos::new(chunk_x, chunk_z));
            }
        }
        PathNavigationRegion { level, snapshot: true }
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        if self.level.chunks.is_outside_build_height(pos.y) {
            return blocks::AIR.default_state();
        }
        if !self.snapshot {
            self.level.chunks.count_get_chunk(pos.chunk());
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

    pub fn get_state_collision_shape_in_context(&self, state: BlockState, pos: BlockPos, context: &CollisionContext) -> Cow<'static, VoxelShape> {
        if state.is_block(blocks::MOVING_PISTON) || state.has_dynamic_shape() && state.block().is_instance_of("ShulkerBoxBlock") {
            self.get_state_collision_shape(state, pos)
        } else {
            context.get_moved_collision_shape(state, pos)
        }
    }

    pub fn no_collision(&self, entity: EntityId, aabb: &AABB) -> bool {
        self.level.no_collision_for(entity, aabb)
    }

    pub fn get_min_y(&self) -> i32 {
        self.level.chunks.min_y()
    }
}
