use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::properties::{DRIED_GHAST_HYDRATION_LEVELS, HORIZONTAL_FACING, WATERLOGGED};
use wafer_data::{Block, BlockState, Fluid, FluidState, registry, registry_id};

use super::simple_waterlogged_block::{self, is_water_source, schedule_water_tick};
use super::{BlockBehaviour, horizontal_directional_block};
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use super::mirror::Mirror;
use super::rotation::Rotation;
use crate::world::level::LevelAccessor;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub const MAX_HYDRATION_LEVEL: i32 = 3;
pub const HYDRATION_TICK_DELAY: i32 = 5000;

fn sound(name: &str) -> i32 {
    registry_id(registry::SOUND_EVENT, name)
}

pub struct DriedGhastBlock {
    block: Block,
}

impl DriedGhastBlock {
    pub fn new(block: Block) -> DriedGhastBlock {
        DriedGhastBlock { block }
    }

    pub fn get_hydration_level(&self, state: BlockState) -> i32 {
        state.get(DRIED_GHAST_HYDRATION_LEVELS)
    }

    fn is_ready_to_spawn(&self, state: BlockState) -> bool {
        self.get_hydration_level(state) == MAX_HYDRATION_LEVEL
    }

    fn tick_waterlogged(&self, state: BlockState, level: &mut ServerLevel, position: BlockPos) {
        if !self.is_ready_to_spawn(state) {
            level.play_sound(None, position, sound("minecraft:block.dried_ghast.transition"), SoundSource::Blocks, 1.0, 1.0);
            level.set_block(position, state.set(DRIED_GHAST_HYDRATION_LEVELS, self.get_hydration_level(state) + 1), UPDATE_CLIENTS);
            level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, position, crate::world::level::gameevent::game_event::Context::of_state(Some(state)));
        } else {
            self.spawn_ghastling(level, position, state);
        }
    }

    fn spawn_ghastling(&self, level: &mut ServerLevel, position: BlockPos, state: BlockState) {
        level.remove_block(position, false);
        let Some(LevelEntity::Mob(mut ghastling)) = ServerLevel::create_entity("minecraft:happy_ghast") else { return };
        let spawn_at = Vec3::at_bottom_center_of(position);
        ghastling.set_baby(true);
        let block_rotation = state.get(HORIZONTAL_FACING).y_rot();
        ghastling.set_y_head_rot(block_rotation);
        ghastling.entity.snap_to(spawn_at.x, spawn_at.y, spawn_at.z, block_rotation, 0.0);
        let id = ghastling.entity.id;
        level.add_fresh_entity(LevelEntity::Mob(ghastling));
        level.play_sound_from_entity(None, id, sound("minecraft:entity.ghastling.spawn"), SoundSource::Blocks, 1.0, 1.0);
    }
}

impl BlockBehaviour for DriedGhastBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, position: BlockPos) {
        if state.get(WATERLOGGED) {
            self.tick_waterlogged(state, level, position);
        } else {
            let hydration_level = self.get_hydration_level(state);
            if hydration_level > 0 {
                level.set_block(position, state.set(DRIED_GHAST_HYDRATION_LEVELS, hydration_level - 1), UPDATE_CLIENTS);
                level.game_event_at_with(crate::world::level::gameevent::game_event::BLOCK_CHANGE, position, crate::world::level::gameevent::game_event::Context::of_state(Some(state)));
            }
        }
    }

    fn random_tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if (state.get(WATERLOGGED) || state.get(DRIED_GHAST_HYDRATION_LEVELS) > 0) && !level.has_scheduled_tick(pos, self.block) {
            level.schedule_tick(pos, self.block, HYDRATION_TICK_DELAY);
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let is_water_source = is_water_source(level, ctx.clicked_pos());
        Some(block.default_state().set(WATERLOGGED, is_water_source).set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }

    fn place_liquid(&self, level: &mut dyn LevelAccessor, pos: BlockPos, state: BlockState, fluid: FluidState) -> bool {
        if !state.get(WATERLOGGED) && fluid.fluid == Fluid::Water {
            let placed = simple_waterlogged_block::place_liquid(level, pos, state, fluid);
            level.play_sound(pos, sound("minecraft:block.dried_ghast.place_in_water"), SoundSource::Blocks, 1.0, 1.0);
            placed
        } else {
            false
        }
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        let name = if state.get(WATERLOGGED) { "minecraft:block.dried_ghast.place_in_water" } else { "minecraft:block.dried_ghast.place" };
        level.play_sound(None, pos, sound(name), SoundSource::Blocks, 1.0, 1.0);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        horizontal_directional_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        horizontal_directional_block::mirror(state, mirror)
    }
}
