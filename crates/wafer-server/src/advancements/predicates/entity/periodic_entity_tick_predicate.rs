use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult};
use crate::world::item::component::codec::Wrap;
use crate::world::entity::EntityId;

pub fn codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::types;
    types::var_int(types::positive_int())
}

#[derive(Debug)]
pub struct PeriodicEntityTickPredicate {
    periodic_tick: i32,
}

impl PeriodicEntityTickPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<PeriodicEntityTickPredicate> {
        codec::wrapped(Wrap::Xmap, || codec::positive_int(tag)).map(|periodic_tick| PeriodicEntityTickPredicate { periodic_tick })
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.entity(entity).tick_count % self.periodic_tick == 0
    }
}
