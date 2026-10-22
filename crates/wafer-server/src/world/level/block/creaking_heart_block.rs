use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{AXIS, CREAKING_HEART_STATE, CreakingHeartState, NATURAL};
use wafer_data::{Block, BlockState, block_tags};

use super::BlockBehaviour;
use super::entity::BlockEntityKind;
use super::rotated_pillar_block;
use super::rotation::Rotation;
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::server_explosion::ServerExplosion;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::level::{BlockGetter, LevelAccessor};

pub struct CreakingHeartBlock;

fn update_state(state: BlockState, level: &mut ServerLevel, pos: BlockPos) -> BlockState {
    let has_logs = has_required_logs(state, level, pos);
    let disabled = state.get(CREAKING_HEART_STATE) == CreakingHeartState::Uprooted;
    if has_logs && disabled {
        let creaking_active = level.environment_attributes().get_value_at(crate::world::attribute::environment_attributes::CREAKING_ACTIVE, pos);
        state.set(CREAKING_HEART_STATE, if creaking_active { CreakingHeartState::Awake } else { CreakingHeartState::Dormant })
    } else {
        state
    }
}

pub fn has_required_logs(state: BlockState, level: &mut (impl BlockGetter + ?Sized), pos: BlockPos) -> bool {
    let axis = state.get(AXIS);
    for dir in [axis.positive(), axis.negative()] {
        let neighbour = level.block_state(pos.relative(dir));
        if !neighbour.is(&block_tags::PALE_OAK_LOGS) || neighbour.get(AXIS) != axis {
            return false;
        }
    }
    true
}

fn try_award_experience(level: &mut ServerLevel, player: EntityId, state: BlockState, pos: BlockPos) {
    if !level.prevents_block_drops(player) && !level.is_spectator(player) && state.get(NATURAL) {
        let amount = level.random.next_int_between_inclusive(20, 24);
        level.pop_experience(pos, amount);
    }
}

impl BlockBehaviour for CreakingHeartBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        level.schedule_tick(pos, state.block(), 1);
        state
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        let new_state = update_state(state, level, pos);
        if new_state != state {
            level.set_block(pos, new_state, UPDATE_ALL);
        }
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        Some(update_state(block.default_state().set(AXIS, ctx.clicked_face().axis()), level, ctx.clicked_pos()))
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        rotated_pillar_block::rotate_pillar(state, rotation)
    }

    fn affect_neighbors_after_removal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _moved_by_piston: bool) {
        level.update_neighbour_for_output_signal(pos, state.block());
    }

    fn on_explosion_hit(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, explosion: &ServerExplosion, stacks: &mut Vec<(BlockPos, crate::world::item::ItemStack)>) {
        if level.get_block_entity(pos).is_some_and(|entity| matches!(entity.kind, BlockEntityKind::CreakingHeart(_))) && explosion.block_interaction.should_affect_blocklike_entities() {
            level.creaking_heart_remove_protector(pos, Some(explosion.damage_source.clone()));
            if let Some(player) = explosion.indirect_source.filter(|&entity| level.is_player(entity)) {
                try_award_experience(level, player, state, pos);
            }
        }
        level.block_on_explosion_hit(state, pos, explosion, stacks);
    }

    fn player_will_destroy(&self, level: &mut ServerLevel, pos: BlockPos, state: BlockState, player: EntityId) -> BlockState {
        if level.get_block_entity(pos).is_some_and(|entity| matches!(entity.kind, BlockEntityKind::CreakingHeart(_))) {
            let source = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(wafer_data::damage_types::PLAYER_ATTACK) };
            level.creaking_heart_remove_protector(pos, Some(source));
            try_award_experience(level, player, state, pos);
        }
        super::block_player_will_destroy(level, pos, state, player)
    }

    fn get_analog_output_signal(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _direction: Direction) -> i32 {
        if state.get(CREAKING_HEART_STATE) == CreakingHeartState::Uprooted {
            return 0;
        }
        match level.get_block_entity(pos).map(|entity| &entity.kind) {
            Some(BlockEntityKind::CreakingHeart(heart)) => heart.get_analog_output_signal(),
            _ => 0,
        }
    }
}
