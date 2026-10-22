use wafer_core::{BlockPos, Direction};
use wafer_data::{BlockState, block_tags, damage_types, mob_effects};

use super::BlockBehaviour;
use super::vegetation_block::{self, Vegetation};
use crate::server::level::ServerLevel;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::EntityId;
use crate::world::entity::inside_block::InsideBlockEffects;
use crate::world::level::{LevelAccessor, LevelReader};

pub struct WitherRoseBlock;

impl Vegetation for WitherRoseBlock {
    fn may_place_on(&self, state: BlockState, _level: &mut dyn LevelReader, _pos: BlockPos) -> bool {
        state.is(&block_tags::SUPPORTS_WITHER_ROSE)
    }
}

impl BlockBehaviour for WitherRoseBlock {
    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        vegetation_block::update_shape(state, level, pos)
    }

    fn can_survive(&self, _state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> bool {
        vegetation_block::can_survive(self, level, pos)
    }

    fn entity_inside(&self, _state: BlockState, level: &mut ServerLevel, _pos: BlockPos, entity: EntityId, _effects: &mut InsideBlockEffects, _is_precise: bool) {
        if level.difficulty != Difficulty::Peaceful && level.is_living(entity) && !level.is_invulnerable_to(entity, &DamageSource::new(damage_types::WITHER)) {
            level.add_effect(entity, get_bee_interaction_effect(), None);
        }
    }
}

pub fn get_bee_interaction_effect() -> MobEffectInstance {
    MobEffectInstance::new(mob_effects::WITHER, 40, 0)
}
