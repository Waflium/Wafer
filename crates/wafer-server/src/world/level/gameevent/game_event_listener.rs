use wafer_core::{BlockPos, Vec3};

use super::game_event::{Context, GameEvent};
use super::position_source::PositionSource;
use super::vibrations::vibration_system::VibrationUser;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DeliveryMode {
    Unspecified,
    ByDistance,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum EntityListenerKind {
    Vibration,
    Jukebox,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum GameEventListener {
    BlockEntity { pos: BlockPos, id: u64 },
    Entity { entity: EntityId, kind: EntityListenerKind },
}

impl ServerLevel {
    pub fn game_event_listener_source(&self, listener: GameEventListener) -> Option<PositionSource> {
        match listener {
            GameEventListener::BlockEntity { pos, .. } => Some(PositionSource::Block(pos)),
            GameEventListener::Entity { entity, kind } => self.entity_game_event_listener_source(entity, kind),
        }
    }

    pub fn game_event_listener_radius(&self, listener: GameEventListener) -> i32 {
        match listener {
            GameEventListener::BlockEntity { pos, id } => self.block_entity_game_event_listener_radius(pos, id),
            GameEventListener::Entity { entity, kind } => self.entity_game_event_listener_radius(entity, kind),
        }
    }

    pub fn game_event_listener_delivery_mode(&self, listener: GameEventListener) -> DeliveryMode {
        match listener {
            GameEventListener::BlockEntity { pos, id } => self.block_entity_game_event_listener_delivery_mode(pos, id),
            GameEventListener::Entity { .. } => DeliveryMode::Unspecified,
        }
    }

    pub fn handle_game_event(&mut self, listener: GameEventListener, event: GameEvent, context: Context, source_position: Vec3) -> bool {
        match listener {
            GameEventListener::BlockEntity { pos, id } => self.block_entity_handle_game_event(pos, id, event, context, source_position),
            GameEventListener::Entity { entity, kind } => self.entity_handle_game_event(entity, kind, event, context, source_position),
        }
    }
}

impl ServerLevel {
    fn block_entity_vibration_user(&self, pos: BlockPos, id: u64) -> Option<VibrationUser> {
        self.sculk_sensor_vibration_user(pos).filter(|user| matches!(user, VibrationUser::SculkSensor { id: user_id, .. } | VibrationUser::CalibratedSculkSensor { id: user_id, .. } if *user_id == id))
    }

    fn block_entity_game_event_listener_radius(&self, pos: BlockPos, id: u64) -> i32 {
        self.block_entity_vibration_user(pos, id).map_or(0, |user| self.vibration_user_listener_radius(user))
    }

    fn block_entity_game_event_listener_delivery_mode(&self, _pos: BlockPos, _id: u64) -> DeliveryMode {
        DeliveryMode::Unspecified
    }

    fn block_entity_handle_game_event(&mut self, pos: BlockPos, id: u64, event: GameEvent, context: Context, source_position: Vec3) -> bool {
        match self.block_entity_vibration_user(pos, id) {
            Some(user) => self.vibration_listener_handle_game_event(user, event, context, source_position),
            None => false,
        }
    }

    fn entity_game_event_listener_source(&self, entity: EntityId, kind: EntityListenerKind) -> Option<PositionSource> {
        if self.is_allay(entity) {
            return Some(self.allay_listener_source(entity, kind));
        }
        None
    }

    fn entity_game_event_listener_radius(&self, entity: EntityId, kind: EntityListenerKind) -> i32 {
        if self.is_allay(entity) {
            return self.allay_listener_radius(kind);
        }
        0
    }

    fn entity_handle_game_event(&mut self, entity: EntityId, kind: EntityListenerKind, event: GameEvent, context: Context, source_position: Vec3) -> bool {
        if self.is_allay(entity) {
            return self.allay_handle_game_event(entity, kind, event, context, source_position);
        }
        false
    }
}
