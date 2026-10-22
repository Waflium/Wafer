use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{SHRIEKING, WATERLOGGED};
use wafer_data::{Block, BlockState, Fluid};

use super::BlockBehaviour;
use super::drop_experience_block::XpRange;
use super::simple_waterlogged_block::schedule_water_tick;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::{BlockGetter, LevelAccessor};

pub struct SculkShriekerBlock;

impl BlockBehaviour for SculkShriekerBlock {
    fn step_on(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, entity: EntityId) {
        if let Some(player) = level.try_get_player(Some(entity)) {
            level.sculk_shrieker_try_shriek(pos, Some(player));
        }
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(SHRIEKING) {
            level.set_block(pos, state.set(SHRIEKING, false), UPDATE_ALL);
            level.sculk_shrieker_try_respond(pos);
        }
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        schedule_water_tick(state, level, pos);
        state
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(WATERLOGGED, level.fluid_state(ctx.clicked_pos()).fluid == Fluid::Water))
    }

    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, drop_experience: bool) {
        if drop_experience {
            level.try_drop_experience(pos, tool.as_ref(), XpRange::Constant(5));
        }
    }
}
