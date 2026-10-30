use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::distance_predicate::{self, DistancePredicate};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    distance_predicate::codec()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct DistanceToPlayerPredicate {
    distance: DistancePredicate,
}

impl DistanceToPlayerPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<DistanceToPlayerPredicate> {
        codec::wrapped(Wrap::Xmap, || DistancePredicate::decode(tag)).map(|distance| DistanceToPlayerPredicate { distance })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId, position: Option<Vec3>) -> bool {
        let Some(position) = position else { return false };
        let e = level.entity(entity);
        self.distance.matches(position.x, position.y, position.z, e.x(), e.y(), e.z())
    }
}
