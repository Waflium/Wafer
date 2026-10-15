use super::EntityId;
use crate::server::level::ServerLevel;

impl ServerLevel {
    pub fn is_player_rideable_jumping(&self, id: EntityId) -> bool {
        self.is_abstract_horse(id)
    }

    pub fn can_jump(&self, id: EntityId) -> bool {
        self.abstract_horse_can_jump(id)
    }

    pub fn handle_start_jump(&mut self, id: EntityId, jump_scale: i32) {
        self.abstract_horse_handle_start_jump(id, jump_scale);
    }

    pub fn handle_stop_jump(&mut self, id: EntityId) {
        self.abstract_horse_handle_stop_jump(id);
    }
}
