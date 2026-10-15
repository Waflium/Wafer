use super::EntityId;
use super::entity_reference::EntityReference;
use super::tamable_animal;
use crate::server::level::ServerLevel;

impl ServerLevel {
    pub fn is_ownable_entity(&self, id: EntityId) -> bool {
        self.is_tamable_animal(id) || self.is_abstract_horse(id)
    }

    pub fn get_owner_reference(&self, id: EntityId) -> Option<EntityReference> {
        if self.is_abstract_horse(id) {
            return self.abstract_horse_get_owner_reference(id);
        }
        tamable_animal::get_owner_reference(self.entity(id))
    }

    pub fn get_owner(&self, id: EntityId) -> Option<EntityId> {
        EntityReference::get_living_entity(self.get_owner_reference(id), self)
    }

    pub fn get_root_owner(&self, id: EntityId) -> Option<EntityId> {
        let mut seen = vec![id];
        let mut owner = self.get_owner(id);
        while let Some(ownable_owner) = owner.filter(|&owner| self.is_ownable_entity(owner)) {
            let owners_owner = self.get_owner(ownable_owner);
            if owners_owner.is_some_and(|owners_owner| seen.contains(&owners_owner)) {
                return None;
            }
            seen.push(ownable_owner);
            owner = self.get_owner(ownable_owner);
        }
        owner
    }
}
