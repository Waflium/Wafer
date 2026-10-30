use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::server::players::old_users_converter::convert_mob_owner_if_necessary;
use crate::world::entity::EntityId;
use crate::world::level::storage::uuid_ints;
use crate::world::level::storage::tag_value_input;

const READ_WITH_OLD_OWNER_CONVERSION: [&str; 3] = [
    "net.minecraft.world.entity.EntityReference.lambda$readWithOldOwnerConversion$0(EntityReference.java:135)",
    "java.base/java.util.Optional.map(Optional.java:260)",
    "net.minecraft.world.entity.EntityReference.readWithOldOwnerConversion(EntityReference.java:135)",
];

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
        level.get_entity_in_any_dimension(self.uuid).filter(|&id| level.entity_ref(id).is_some_and(|e| !e.is_removed()))
    }

    pub fn get_player(self, level: &ServerLevel) -> Option<EntityId> {
        level.get_player_in_any_dimension(self.uuid).filter(|&id| level.entity_ref(id).is_some_and(|e| !e.is_removed()))
    }

    pub fn get_entity_in_any_dimension(self, level: &ServerLevel) -> Option<(&ServerLevel, EntityId)> {
        self.get_entity(level).map(|id| (level.level_holding(id), id))
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

    pub fn read_with_old_owner_conversion(input: &Nbt, key: &str, level: &ServerLevel, callers: &[&'static str]) -> Option<EntityReference> {
        EntityReference::read(input, key).or_else(|| {
            let old_name = input.get(key).and_then(Nbt::as_str)?;
            let frames: Vec<&'static str> = READ_WITH_OLD_OWNER_CONVERSION.into_iter().chain(callers.iter().copied()).collect();
            convert_mob_owner_if_necessary(level.user_lookup.as_ref()?, old_name, &frames).map(EntityReference::of)
        })
    }

    pub fn get_player_entity(reference: Option<EntityReference>, level: &ServerLevel) -> Option<EntityId> {
        reference?.get_player(level)
    }

    pub fn get_living_entity(reference: Option<EntityReference>, level: &ServerLevel) -> Option<EntityId> {
        reference?.get_entity(level).filter(|&id| level.is_living(id))
    }

    pub fn get_living_entity_in_any_dimension(reference: Option<EntityReference>, level: &ServerLevel) -> Option<(&ServerLevel, EntityId)> {
        reference?.get_entity_in_any_dimension(level).filter(|&(level, id)| level.is_living(id))
    }
}
