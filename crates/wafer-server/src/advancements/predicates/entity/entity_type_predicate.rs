use wafer_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

pub fn codec() -> crate::world::item::component::types::Pair {
    crate::world::item::component::types::holder_set("minecraft:entity_type")
}

#[derive(Debug)]
pub struct EntityTypePredicate {
    types: HolderSet,
}

impl EntityTypePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityTypePredicate> {
        codec::wrapped(Wrap::Xmap, || HolderSet::decode("minecraft:entity_type", tag)).map(|types| EntityTypePredicate { types })
    }

    pub fn matches_type(&self, entity_type: i32) -> bool {
        self.types.contains(entity_type)
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        self.matches_type(level.entity(entity).entity_type)
    }
}
