use super::EntityId;
use crate::server::level::ServerLevel;

impl ServerLevel {
    pub fn has_custom_inventory_screen(&self, id: EntityId) -> bool {
        self.is_container_entity(id) || self.is_abstract_horse(id)
    }
}
