use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::{BlockState, damage_types};
use wafer_data::properties::BLOOM;
use wafer_protocol::nbt::Nbt;

use super::BlockEntityKind;
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::{ServerLevel, SoundSource};
use crate::world::damagesource::DamageSource;
use crate::world::entity::{EntityId, sound_event};
use crate::world::level::block::sculk_spreader::SculkSpreader;
use crate::world::level::gameevent::game_event::{self, Context, GameEvent};
use crate::world::level::update_flags::UPDATE_ALL;

pub const PULSE_TICKS: i32 = 8;
pub const LISTENER_RADIUS: i32 = 8;

#[derive(Clone)]
pub struct CatalystListener {
    sculk_spreader: SculkSpreader,
    block_state: BlockState,
}

#[derive(Clone)]
pub struct SculkCatalystBlockEntity {
    pub catalyst_listener: CatalystListener,
}

impl SculkCatalystBlockEntity {
    pub fn new(block_state: BlockState) -> SculkCatalystBlockEntity {
        SculkCatalystBlockEntity { catalyst_listener: CatalystListener { sculk_spreader: SculkSpreader::create_level_spreader(), block_state } }
    }

    pub fn load_additional(&mut self, input: &Nbt) {
        self.catalyst_listener.sculk_spreader.load(input);
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        self.catalyst_listener.sculk_spreader.save(output);
    }
}

impl ServerLevel {
    fn sculk_catalyst_mut(&mut self, pos: BlockPos, id: u64) -> Option<&mut SculkCatalystBlockEntity> {
        match self.get_block_entity(pos).filter(|entity| entity.id() == id).map(|entity| &mut entity.kind) {
            Some(BlockEntityKind::SculkCatalyst(catalyst)) => Some(catalyst),
            _ => None,
        }
    }

    pub fn sculk_catalyst_server_tick(&mut self, pos: BlockPos) {
        let Some(id) = self.chunks.block_entity_ref(pos).map(|entity| entity.id()) else { return };
        let Some(catalyst) = self.sculk_catalyst_mut(pos, id) else { return };
        let mut sculk_spreader = std::mem::replace(&mut catalyst.catalyst_listener.sculk_spreader, SculkSpreader::create_level_spreader());
        self.with_level_random(|level, random| sculk_spreader.update_cursors(level, pos, random, true));
        if let Some(catalyst) = self.sculk_catalyst_mut(pos, id) {
            catalyst.catalyst_listener.sculk_spreader = sculk_spreader;
        }
    }

    pub fn catalyst_listener_handle_game_event(&mut self, pos: BlockPos, id: u64, event: GameEvent, context: Context, source_position: Vec3) -> bool {
        let Some(mob) = context.source_entity.filter(|&entity| event == game_event::ENTITY_DIE && self.entity_ref(entity).is_some() && self.is_living(entity)) else { return false };
        if !self.was_experience_consumed(mob) {
            let killer = self.get_last_damage_source(mob).and_then(|source| source.causing_entity);
            let experience_would_drop = self.get_experience_reward(mob, killer);
            if self.should_drop_experience(mob) && experience_would_drop > 0 {
                if let Some(catalyst) = self.sculk_catalyst_mut(pos, id) {
                    catalyst.catalyst_listener.sculk_spreader.add_cursors(BlockPos::containing_vec(source_position.relative(Direction::Up, 0.5)), experience_would_drop);
                }
                self.catalyst_listener_try_award_it_spreads_advancement(mob);
            }
            self.skip_drop_experience(mob);
            if let Some(block_state) = self.sculk_catalyst_mut(pos, id).map(|catalyst| catalyst.catalyst_listener.block_state) {
                self.catalyst_listener_bloom(pos, block_state);
            }
        }
        true
    }

    fn catalyst_listener_bloom(&mut self, pos: BlockPos, state: BlockState) {
        self.set_block(pos, state.set(BLOOM, true), UPDATE_ALL);
        self.schedule_tick(pos, state.block(), PULSE_TICKS);
        self.send_particles("minecraft:sculk_soul", |_| {}, pos.x as f64 + 0.5, pos.y as f64 + 1.15, pos.z as f64 + 0.5, 2, (0.2, 0.0, 0.2), 0.0);
        let pitch = 0.6 + self.random.next_float() * 0.4;
        self.play_sound(None, pos, sound_event("minecraft:block.sculk_catalyst.bloom"), SoundSource::Blocks, 2.0, pitch);
    }

    fn catalyst_listener_try_award_it_spreads_advancement(&mut self, mob: EntityId) {
        let Some(player) = self.get_last_hurt_by_mob(mob).filter(|&entity| self.is_player(entity)) else { return };
        let damage_source = self.get_last_damage_source(mob).cloned().unwrap_or_else(|| DamageSource::entity(damage_types::PLAYER_ATTACK, player));
        criteria_triggers::KILL_MOB_NEAR_SCULK_CATALYST.trigger(self, player, mob, &damage_source);
    }
}
