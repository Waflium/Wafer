use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::entity_predicate::{self, EntityPredicate};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    entity_predicate::codec()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct VehiclePredicate {
    vehicle: Box<EntityPredicate>,
}

impl VehiclePredicate {
    pub fn decode(tag: &Nbt) -> DataResult<VehiclePredicate> {
        codec::wrapped(Wrap::Xmap, || EntityPredicate::decode(tag)).map(|vehicle| VehiclePredicate { vehicle: Box::new(vehicle) })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let vehicle = level.vehicle(entity);
        self.vehicle.matches(level, position, vehicle)
    }
}
