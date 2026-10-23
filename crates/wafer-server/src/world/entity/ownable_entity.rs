use super::EntityId;
use super::entity_reference::EntityReference;
use super::tamable_animal;
use crate::server::level::ServerLevel;

impl ServerLevel {
    pub fn is_ownable_entity(&self, id: EntityId) -> bool {
        self.is_tamable_animal(id) || self.is_abstract_horse(id) || self.is_vex(id)
    }

    pub fn get_owner_reference(&self, id: EntityId) -> Option<EntityReference> {
        if self.is_abstract_horse(id) {
            return self.abstract_horse_get_owner_reference(id);
        }
        if self.is_vex(id) {
            return super::monster::vex::Vex::get_owner_reference(self.mob(id).expect("a vex"));
        }
        tamable_animal::get_owner_reference(self.entity(id))
    }

    pub fn get_owner(&self, id: EntityId) -> Option<EntityId> {
        EntityReference::get_living_entity(self.get_owner_reference(id), self)
    }

    pub fn get_owner_in_any_dimension(&self, id: EntityId) -> Option<(&ServerLevel, EntityId)> {
        EntityReference::get_living_entity_in_any_dimension(self.get_owner_reference(id), self)
    }

    pub fn get_root_owner_in_any_dimension(&self, id: EntityId) -> Option<(&ServerLevel, EntityId)> {
        let mut seen = vec![(self.index, id)];
        let mut owner = self.get_owner_in_any_dimension(id);
        while let Some((owner_level, ownable_owner)) = owner.filter(|&(owner_level, owner)| owner_level.is_ownable_entity(owner)) {
            let owners_owner = EntityReference::get_living_entity_in_any_dimension(owner_level.get_owner_reference(ownable_owner), self);
            if owners_owner.is_some_and(|(owners_owner_level, owners_owner)| seen.contains(&(owners_owner_level.index, owners_owner))) {
                return None;
            }
            seen.push((owner_level.index, ownable_owner));
            owner = owners_owner;
        }
        owner
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
