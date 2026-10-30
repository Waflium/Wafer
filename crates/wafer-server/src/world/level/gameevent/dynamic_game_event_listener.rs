use wafer_core::{ChunkPos, SectionPos};

use super::game_event_listener::GameEventListener;
use crate::server::level::ServerLevel;

#[derive(Clone, Copy, Debug)]
pub struct DynamicGameEventListener {
    pub listener: GameEventListener,
    last_section: Option<(i32, i32, i32)>,
}

impl DynamicGameEventListener {
    pub fn new(listener: GameEventListener) -> DynamicGameEventListener {
        DynamicGameEventListener { listener, last_section: None }
    }
}

fn section_of(level: &ServerLevel, listener: GameEventListener) -> Option<(i32, i32, i32)> {
    let position = level.game_event_listener_source(listener)?.get_position(level)?;
    let pos = wafer_core::BlockPos::containing_vec(position);
    Some((SectionPos::block_to_section_coord(pos.x), SectionPos::block_to_section_coord(pos.y), SectionPos::block_to_section_coord(pos.z)))
}

impl ServerLevel {
    pub fn dynamic_game_event_listener_add(&mut self, dynamic: &mut DynamicGameEventListener) {
        self.dynamic_game_event_listener_move(dynamic);
    }

    pub fn dynamic_game_event_listener_remove(&mut self, dynamic: &mut DynamicGameEventListener) {
        if let Some((x, y, z)) = dynamic.last_section
            && let Some(chunk) = self.chunks.chunk_without_loading_mut(ChunkPos::new(x, z))
        {
            chunk.unregister_game_event_listener(y, dynamic.listener);
        }
    }

    pub fn dynamic_game_event_listener_move(&mut self, dynamic: &mut DynamicGameEventListener) {
        let Some(current_section) = section_of(self, dynamic.listener) else { return };
        if dynamic.last_section != Some(current_section) {
            self.dynamic_game_event_listener_remove(dynamic);
            dynamic.last_section = Some(current_section);
            let (x, y, z) = current_section;
            if let Some(chunk) = self.chunks.chunk_without_loading_mut(ChunkPos::new(x, z)) {
                chunk.get_listener_registry(y).register(dynamic.listener);
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DynamicListenerAction {
    Add,
    Remove,
    Move,
}

impl ServerLevel {
    fn dynamic_game_event_listeners(&self, id: crate::world::entity::EntityId) -> Vec<DynamicGameEventListener> {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(crate::world::entity::mob::MobKind::Allay(allay)) => vec![allay.dynamic_vibration_listener, allay.dynamic_jukebox_listener],
            Some(crate::world::entity::mob::MobKind::Warden(warden)) => vec![warden.dynamic_game_event_listener],
            _ => Vec::new(),
        }
    }

    fn set_dynamic_game_event_listener(&mut self, id: crate::world::entity::EntityId, index: usize, dynamic: DynamicGameEventListener) {
        match self.mob_mut(id).map(|mob| &mut mob.kind) {
            Some(crate::world::entity::mob::MobKind::Allay(allay)) => {
                if index == 0 {
                    allay.dynamic_vibration_listener = dynamic;
                } else {
                    allay.dynamic_jukebox_listener = dynamic;
                }
            }
            Some(crate::world::entity::mob::MobKind::Warden(warden)) => warden.dynamic_game_event_listener = dynamic,
            _ => {}
        }
    }

    pub fn update_dynamic_game_event_listener(&mut self, id: crate::world::entity::EntityId, action: DynamicListenerAction) {
        for (index, mut dynamic) in self.dynamic_game_event_listeners(id).into_iter().enumerate() {
            match action {
                DynamicListenerAction::Add => self.dynamic_game_event_listener_add(&mut dynamic),
                DynamicListenerAction::Remove => self.dynamic_game_event_listener_remove(&mut dynamic),
                DynamicListenerAction::Move => self.dynamic_game_event_listener_move(&mut dynamic),
            }
            self.set_dynamic_game_event_listener(id, index, dynamic);
        }
    }
}
