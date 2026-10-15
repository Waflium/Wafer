use super::EntityId;
use crate::server::level::ServerLevel;

impl ServerLevel {
    pub fn is_item_steerable(&self, id: EntityId) -> bool {
        self.is_pig(id)
    }

    pub fn boost(&mut self, id: EntityId) -> bool {
        self.pig_boost(id)
    }
}
