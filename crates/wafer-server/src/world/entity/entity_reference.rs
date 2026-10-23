use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::server::players::old_users_converter::convert_mob_owner_if_necessary;
use crate::world::entity::EntityId;
use crate::world::level::storage::uuid_ints;
use crate::world::level::storage::tag_value_input;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct EntityReference {
    uuid: u128,
}

impl EntityReference {
    pub fn of(uuid: u128) -> EntityReference {
        EntityReference { uuid }
    }

    pub fn get_uuid(self) -> u128 {
        self.uuid
    }

    pub fn matches(self, level: &ServerLevel, entity: EntityId) -> bool {
        level.entity(entity).uuid == self.uuid
    }

    pub fn get_entity(self, level: &ServerLevel) -> Option<EntityId> {
        level.entity_manager.visible_entity_storage.get(self.uuid).filter(|&id| level.entity_ref(id).is_some_and(|e| !e.is_removed()))
    }

    pub fn get_entity_in_any_dimension(self, level: &ServerLevel) -> Option<(&ServerLevel, EntityId)> {
        if let Some(id) = self.get_entity(level) {
            return Some((level, id));
        }
        level.other_levels.iter().find_map(|other_level| self.get_entity(other_level).map(|id| (other_level, id)))
    }

    pub fn store(reference: Option<EntityReference>, output: &mut Vec<(String, Nbt)>, key: &str) {
        if let Some(reference) = reference {
            output.push((key.into(), uuid_ints(reference.get_uuid())));
        }
    }

    pub fn read(input: &Nbt, key: &str) -> Option<EntityReference> {
        tag_value_input::read(input, key, crate::world::entity::entity_save::decode_uuid).map(EntityReference::of)
    }

    pub fn read_reporting_old_owner(input: &Nbt, key: &str) -> Option<EntityReference> {
        EntityReference::read(input, key).or_else(|| {
            tag_value_input::get_string(input, key);
            None
        })
    }

    pub fn read_with_old_owner_conversion(input: &Nbt, key: &str, level: &ServerLevel) -> Option<EntityReference> {
        EntityReference::read(input, key).or_else(|| {
            let old_name = input.get(key).and_then(Nbt::as_str)?;
            convert_mob_owner_if_necessary(level.user_lookup.as_ref()?, old_name).map(EntityReference::of)
        })
    }

    pub fn get_living_entity(reference: Option<EntityReference>, level: &ServerLevel) -> Option<EntityId> {
        reference?.get_entity(level).filter(|&id| level.is_living(id))
    }

    pub fn get_living_entity_in_any_dimension(reference: Option<EntityReference>, level: &ServerLevel) -> Option<(&ServerLevel, EntityId)> {
        reference?.get_entity_in_any_dimension(level).filter(|&(level, id)| level.is_living(id))
    }
}
