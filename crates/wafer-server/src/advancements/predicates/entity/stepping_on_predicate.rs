use wafer_core::Vec3;
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
pub struct SteppingOnPredicate {
    predicate: LocationPredicate,
}

impl SteppingOnPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<SteppingOnPredicate> {
        codec::wrapped(Wrap::Xmap, || LocationPredicate::decode(tag)).map(|predicate| SteppingOnPredicate { predicate })
    }

    pub fn matches(&self, level: &mut ServerLevel, entity: EntityId) -> bool {
        if !level.entity(entity).on_ground {
            return false;
        }
        let on_pos = Vec3::at_center_of(level.on_pos(entity, 1.0e-5));
        self.predicate.matches(level, on_pos.x, on_pos.y, on_pos.z)
    }
}
