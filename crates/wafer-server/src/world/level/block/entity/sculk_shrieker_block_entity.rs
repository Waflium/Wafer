use wafer_core::{BlockPos, Vec3, mth};
use wafer_data::properties::{CAN_SUMMON, SHRIEKING};
use wafer_data::game_rule;
use wafer_protocol::nbt::Nbt;

use super::{BlockEntityKind, int_value};
use crate::server::level::events::PARTICLES_SCULK_SHRIEK;
use crate::server::level::{ServerLevel, SoundSource};
use crate::util::spawn_util::{self, Strategy};
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_types;
use crate::world::entity::monster::warden::warden;
use crate::world::entity::monster::warden::warden_spawn_tracker;
use crate::world::entity::{EntityId, LevelEntity, sound_event};
use crate::world::level::gameevent::game_event::{self, Context};
use crate::world::level::gameevent::vibrations::vibration_system::{Data, VibrationUser};
use crate::world::level::update_flags::UPDATE_CLIENTS;
use crate::world::difficulty::Difficulty;
use crate::world::level::BlockGetter;
use crate::world::level::storage::tag_value_input;

const WARDEN_SPAWN_ATTEMPTS: i32 = 20;
const WARDEN_SPAWN_RANGE_XZ: i32 = 5;
const WARDEN_SPAWN_RANGE_Y: i32 = 6;
const DARKNESS_RADIUS: i32 = 40;
const SHRIEKING_TICKS: i32 = 90;
const DEFAULT_WARNING_LEVEL: i32 = 0;
pub const LISTENER_RADIUS: i32 = 8;

fn sound_by_level(warning_level: i32) -> Option<&'static str> {
    match warning_level {
        1 => Some("minecraft:entity.warden.nearby_close"),
        2 => Some("minecraft:entity.warden.nearby_closer"),
        3 => Some("minecraft:entity.warden.nearby_closest"),
        4 => Some("minecraft:entity.warden.listening_angry"),
        _ => None,
    }
}

#[derive(Clone, Default)]
pub struct SculkShriekerBlockEntity {
    pub warning_level: i32,
    pub vibration_data: Data,
}

impl SculkShriekerBlockEntity {
    pub fn load_additional(&mut self, input: &Nbt) {
        self.warning_level = tag_value_input::get_numeric(input, "warning_level").and_then(int_value).unwrap_or(DEFAULT_WARNING_LEVEL);
        self.vibration_data = tag_value_input::read(input, "listener", Data::decode).unwrap_or_default();
    }

    pub fn save_additional(&self, output: &mut Vec<(String, Nbt)>) {
        output.push(("warning_level".into(), Nbt::Int(self.warning_level)));
        output.push(("listener".into(), self.vibration_data.to_nbt()));
    }
}

impl ServerLevel {
    fn sculk_shrieker(&self, pos: BlockPos, id: u64) -> Option<&SculkShriekerBlockEntity> {
        match self.chunks.block_entity_ref(pos).filter(|entity| entity.id() == id).map(|entity| &entity.kind) {
            Some(BlockEntityKind::SculkShrieker(shrieker)) => Some(shrieker),
            _ => None,
        }
    }

    fn sculk_shrieker_mut(&mut self, pos: BlockPos) -> Option<&mut SculkShriekerBlockEntity> {
        match self.get_block_entity(pos).map(|entity| &mut entity.kind) {
            Some(BlockEntityKind::SculkShrieker(shrieker)) => Some(shrieker),
            _ => None,
        }
    }

    pub fn sculk_shrieker_vibration_data(&self, pos: BlockPos, id: u64) -> Option<&Data> {
        self.sculk_shrieker(pos, id).map(|shrieker| &shrieker.vibration_data)
    }

    pub fn sculk_shrieker_vibration_data_mut(&mut self, pos: BlockPos, id: u64) -> Option<&mut Data> {
        match self.get_block_entity(pos).filter(|entity| entity.id() == id).map(|entity| &mut entity.kind) {
            Some(BlockEntityKind::SculkShrieker(shrieker)) => Some(&mut shrieker.vibration_data),
            _ => None,
        }
    }

    pub fn sculk_shrieker_vibration_user(&self, pos: BlockPos) -> Option<VibrationUser> {
        let entity = self.chunks.block_entity_ref(pos)?;
        matches!(entity.kind, BlockEntityKind::SculkShrieker(_)).then(|| VibrationUser::SculkShrieker { pos, id: entity.id() })
    }

    pub fn try_get_player(&self, source_entity: Option<EntityId>) -> Option<EntityId> {
        let source_entity = source_entity.filter(|&entity| self.entity_ref(entity).is_some())?;
        if self.is_player(source_entity) {
            return Some(source_entity);
        }
        if let Some(player) = self.controlling_passenger(source_entity).filter(|&passenger| self.is_player(passenger)) {
            return Some(player);
        }
        if self.is_projectile(source_entity) {
            return self.projectile_get_owner(source_entity).filter(|&owner| self.is_player(owner));
        }
        match self.find_entity(source_entity) {
            Some(LevelEntity::Item(item)) => item.thrower.and_then(|uuid| crate::world::entity::entity_reference::EntityReference::of(uuid).get_entity(self)).filter(|&owner| self.is_player(owner)),
            _ => None,
        }
    }

    pub fn sculk_shrieker_try_shriek(&mut self, pos: BlockPos, player: Option<EntityId>) {
        let Some(player) = player else { return };
        let state = self.block_state(pos);
        if state.get(SHRIEKING) {
            return;
        }
        let Some(shrieker) = self.sculk_shrieker_mut(pos) else { return };
        shrieker.warning_level = 0;
        if !self.sculk_shrieker_can_respond(pos) || self.sculk_shrieker_try_to_warn(pos, player) {
            self.sculk_shrieker_shriek(pos, Some(player));
        }
    }

    fn sculk_shrieker_try_to_warn(&mut self, pos: BlockPos, player: EntityId) -> bool {
        let maybe_warning_level = warden_spawn_tracker::try_warn(self, pos, player);
        if let Some(warning_level) = maybe_warning_level
            && let Some(shrieker) = self.sculk_shrieker_mut(pos)
        {
            shrieker.warning_level = warning_level;
        }
        maybe_warning_level.is_some()
    }

    fn sculk_shrieker_shriek(&mut self, pos: BlockPos, source_entity: Option<EntityId>) {
        let state = self.block_state(pos);
        self.set_block(pos, state.set(SHRIEKING, true), UPDATE_CLIENTS);
        self.schedule_tick(pos, state.block(), SHRIEKING_TICKS);
        self.level_event(None, PARTICLES_SCULK_SHRIEK, pos, 0);
        self.game_event_at_with(game_event::SHRIEK, pos, Context::of(source_entity));
    }

    fn sculk_shrieker_can_respond(&mut self, pos: BlockPos) -> bool {
        self.block_state(pos).get(CAN_SUMMON) && self.difficulty != Difficulty::Peaceful && self.game_rules.get_bool(game_rule::SPAWN_WARDENS)
    }

    pub fn sculk_shrieker_pre_remove_side_effects(&mut self, pos: BlockPos, state: wafer_data::BlockState) {
        if state.get(SHRIEKING) {
            self.sculk_shrieker_try_respond(pos);
        }
    }

    pub fn sculk_shrieker_try_respond(&mut self, pos: BlockPos) {
        let warning_level = self.sculk_shrieker_mut(pos).map_or(0, |shrieker| shrieker.warning_level);
        if self.sculk_shrieker_can_respond(pos) && warning_level > 0 {
            if !self.sculk_shrieker_try_summon_warden(pos, warning_level) {
                self.sculk_shrieker_play_warden_reply_sound(pos, warning_level);
            }
            warden::apply_darkness_around(self, Vec3::at_center_of(pos), None, DARKNESS_RADIUS);
        }
    }

    fn sculk_shrieker_play_warden_reply_sound(&mut self, pos: BlockPos, warning_level: i32) {
        let Some(sound) = sound_by_level(warning_level) else { return };
        let x = pos.x + mth::random_between_inclusive(&mut self.random, -10, 10);
        let y = pos.y + mth::random_between_inclusive(&mut self.random, -10, 10);
        let z = pos.z + mth::random_between_inclusive(&mut self.random, -10, 10);
        self.play_sound_at(None, x as f64, y as f64, z as f64, sound_event(sound), SoundSource::Hostile, 5.0, 1.0);
    }

    fn sculk_shrieker_try_summon_warden(&mut self, pos: BlockPos, warning_level: i32) -> bool {
        warning_level >= 4
            && spawn_util::try_spawn_mob(self, &entity_types::WARDEN, EntitySpawnReason::Triggered, pos, WARDEN_SPAWN_ATTEMPTS, WARDEN_SPAWN_RANGE_XZ, WARDEN_SPAWN_RANGE_Y, Strategy::OnTopOfCollider, false).is_some()
    }

    pub fn sculk_shrieker_can_receive_vibration(&self, block_pos: BlockPos, context: Context) -> bool {
        !self.chunks.loaded_chunk(block_pos.chunk()).is_some_and(|chunk| chunk.block_state(block_pos).get(SHRIEKING)) && self.try_get_player(context.source_entity).is_some()
    }

    pub fn sculk_shrieker_on_receive_vibration(&mut self, block_pos: BlockPos, source_entity: Option<EntityId>, projectile_owner: Option<EntityId>) {
        let player = self.try_get_player(projectile_owner.or(source_entity));
        self.sculk_shrieker_try_shriek(block_pos, player);
    }
}
