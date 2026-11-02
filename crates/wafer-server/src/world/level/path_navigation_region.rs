use std::borrow::Cow;

use wafer_core::shapes::VoxelShape;
use wafer_core::{AABB, BlockPos, ChunkPos};
use wafer_data::{BlockState, FluidState, blocks};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::block::entity::{BlockEntity, BlockEntityKind};
use crate::world::level::block::{moving_piston_block, shulker_box_block};
use crate::world::level::chunk::LevelChunk;
use crate::world::level::collision::for_each_block_collision;
use crate::world::phys::shapes::collision_context::CollisionContext;

#[derive(Clone, Copy)]
pub struct PathNavigationRegion<'a> {
    pub level: &'a ServerLevel,
    chunks: Option<(ChunkPos, ChunkPos)>,
}

impl<'a> PathNavigationRegion<'a> {
    pub fn new(level: &'a ServerLevel) -> PathNavigationRegion<'a> {
        PathNavigationRegion { level, chunks: None }
    }

    pub fn snapshot(level: &'a ServerLevel, start: BlockPos, end: BlockPos) -> PathNavigationRegion<'a> {
        for chunk_x in (start.x >> 4)..=(end.x >> 4) {
            for chunk_z in (start.z >> 4)..=(end.z >> 4) {
                level.chunks.get_chunk_now(ChunkPos::new(chunk_x, chunk_z));
            }
        }
        PathNavigationRegion { level, chunks: Some((start.chunk(), end.chunk())) }
    }

    fn get_chunk(&self, pos: ChunkPos) -> Option<&'a LevelChunk> {
        let (center, end) = self.chunks?;
        if pos.x >= center.x && pos.x <= end.x && pos.z >= center.z && pos.z <= end.z { self.level.chunks.loaded_chunk(pos) } else { None }
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        if self.chunks.is_some() {
            if self.level.chunks.is_outside_build_height(pos.y) {
                return blocks::AIR.default_state();
            }
            return self.get_chunk(pos.chunk()).map_or(blocks::VOID_AIR.default_state(), |chunk| chunk.block_state(pos));
        }
        if !self.level.is_in_valid_bounds(pos) {
            return blocks::VOID_AIR.default_state();
        }
        self.level.chunks.count_get_chunk(pos.chunk());
        self.level.chunks.loaded_chunk(pos.chunk()).map_or(blocks::VOID_AIR.default_state(), |chunk| chunk.block_state(pos))
    }

    pub fn get_fluid_state(&self, pos: BlockPos) -> FluidState {
        self.get_block_state(pos).fluid_state()
    }

    fn get_block_entity(&self, pos: BlockPos) -> Option<&'a BlockEntity> {
        if self.chunks.is_some() {
            return self.get_chunk(pos.chunk())?.block_entities.map.get(&pos).filter(|entity| !entity.is_removed());
        }
        if !self.level.is_in_valid_bounds(pos) {
            return None;
        }
        self.level.chunks.count_get_chunk(pos.chunk());
        self.level.chunks.block_entity_ref(pos)
    }

    pub fn get_state_collision_shape(&self, state: BlockState, pos: BlockPos) -> Cow<'static, VoxelShape> {
        if state.is_block(blocks::MOVING_PISTON) {
            let Some(BlockEntity { kind: BlockEntityKind::PistonMoving(entity), .. }) = self.get_block_entity(pos) else {
                return Cow::Owned(VoxelShape::empty());
            };
            Cow::Owned(moving_piston_block::get_collision_shape_of(entity, pos))
        } else if state.has_dynamic_shape() && state.block().is_instance_of("ShulkerBoxBlock") {
            let shulker = match self.get_block_entity(pos).map(|entity| &entity.kind) {
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
        if self.chunks.is_none() {
            return self.level.no_collision_for(entity, aabb);
        }
        for_each_block_collision(|pos| self.get_chunk(pos), aabb, &CollisionContext::of(self.level, entity), false, false, |_, collider| collider.shape.is_empty())
            && self.level.no_border_collision(entity, aabb)
    }

    pub fn get_min_y(&self) -> i32 {
        self.level.chunks.min_y()
    }
}

impl BlockGetter for PathNavigationRegion<'_> {
    fn block_state(&mut self, pos: BlockPos) -> BlockState {
        self.get_block_state(pos)
    }

    fn get_min_y(&self) -> i32 {
        PathNavigationRegion::get_min_y(self)
    }

    fn get_state_collision_shape(&mut self, state: BlockState, pos: BlockPos) -> Cow<'static, VoxelShape> {
        PathNavigationRegion::get_state_collision_shape(self, state, pos)
    }

    fn get_state_collision_shape_in_context(&mut self, state: BlockState, pos: BlockPos, context: &CollisionContext) -> Cow<'static, VoxelShape> {
        PathNavigationRegion::get_state_collision_shape_in_context(self, state, pos, context)
    }
}
