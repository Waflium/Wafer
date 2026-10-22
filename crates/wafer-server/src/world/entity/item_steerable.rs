use super::EntityId;
use crate::server::level::ServerLevel;

impl ServerLevel {
    pub fn is_item_steerable(&self, id: EntityId) -> bool {
        self.is_pig(id) || self.is_strider(id)
    }

    pub fn boost(&mut self, id: EntityId) -> bool {
        if self.is_strider(id) { self.strider_boost(id) } else { self.pig_boost(id) }
    }
}
