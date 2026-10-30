use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::location_predicate::{self, LocationPredicate};
use crate::world::item::component::types::Pair;

pub fn codec() -> Pair {
    location_predicate::codec()
}
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

#[derive(Debug)]
pub struct EntityLocationPredicate {
    predicate: LocationPredicate,
}

impl EntityLocationPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<EntityLocationPredicate> {
        codec::wrapped(Wrap::Xmap, || LocationPredicate::decode(tag)).map(|predicate| EntityLocationPredicate { predicate })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId) -> bool {
        let e = level.entity(entity);
        let (x, y, z) = (e.x(), e.y(), e.z());
        self.predicate.matches(level, x, y, z)
    }
}
