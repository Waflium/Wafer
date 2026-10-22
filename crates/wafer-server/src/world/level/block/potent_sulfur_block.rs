use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{POTENT_SULFUR_STATE, PotentSulfurState};
use wafer_data::{Block, BlockState, Fluid, block_tags};

use super::BlockBehaviour;
use super::entity::BlockEntityKind;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::entity::sound_event;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::gameevent::game_event;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct PotentSulfurBlock;

fn valid_block_state(state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> BlockState {
    if !level.fluid_state(pos.above()).is_source_of_type(Fluid::Water) {
        return state.set(POTENT_SULFUR_STATE, PotentSulfurState::Dry);
    }
    let below_state = level.block_state(pos.below());
    if below_state.is(&block_tags::CAUSES_CONTINUOUS_GEYSER_ERUPTIONS) && is_source_if_fluid(below_state) {
        return state.set(POTENT_SULFUR_STATE, PotentSulfurState::Continuous);
    }
    if below_state.is(&block_tags::CAUSES_PERIODIC_GEYSER_ERUPTIONS) && is_source_if_fluid(below_state) {
        let current = state.get(POTENT_SULFUR_STATE);
        let is_geyser = current == PotentSulfurState::Erupting || current == PotentSulfurState::Dormant;
        if !is_geyser
            && let Some(entity) = level.get_block_entity(pos)
            && let BlockEntityKind::PotentSulfur(potent_sulfur_entity) = &mut entity.kind
        {
            potent_sulfur_entity.reset_countdown();
        }
        if current == PotentSulfurState::Erupting { state } else { state.set(POTENT_SULFUR_STATE, PotentSulfurState::Dormant) }
    } else {
        state.set(POTENT_SULFUR_STATE, PotentSulfurState::Wet)
    }
}

fn is_source_if_fluid(below_state: BlockState) -> bool {
    let fluid_state = below_state.fluid_state();
    fluid_state.is_empty() || fluid_state.is_source()
}

impl BlockBehaviour for PotentSulfurBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        valid_block_state(state, level, pos)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(valid_block_state(block.default_state(), level, ctx.clicked_pos()))
    }

    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        let current = state.get(POTENT_SULFUR_STATE);
        if current == PotentSulfurState::Erupting || current == PotentSulfurState::Continuous {
            level.block_event(pos, state.block(), 0, 0);
            let sound = if current == PotentSulfurState::Continuous { "minecraft:block.potent_sulfur.geyser_continuous_eruption" } else { "minecraft:block.potent_sulfur.geyser_eruption" };
            level.play_sound(None, pos, sound_event(sound), SoundSource::Blocks, 1.0, 1.0);
            level.game_event_at_with(game_event::BLOCK_ACTIVATE, pos, game_event::Context::of_state(Some(state)));
        }
    }

    fn trigger_event(&self, _state: BlockState, _level: &mut ServerLevel, _pos: BlockPos, _b0: i32, _b1: i32) -> bool {
        true
    }
}
