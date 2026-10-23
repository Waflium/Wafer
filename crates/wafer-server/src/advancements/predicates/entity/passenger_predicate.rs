use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::entity_predicate::{self, EntityPredicate};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    entity_predicate::codec()
}
use crate::server::level::ServerLevel;
use crate::util::codec::DataResult;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct PassengerPredicate {
    passenger: Box<EntityPredicate>,
}

impl PassengerPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<PassengerPredicate> {
        Ok(PassengerPredicate { passenger: Box::new(EntityPredicate::decode(tag)?) })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let passengers = level.passengers(entity).to_vec();
        passengers.into_iter().any(|passenger| self.passenger.matches(level, position, Some(passenger)))
    }
}
