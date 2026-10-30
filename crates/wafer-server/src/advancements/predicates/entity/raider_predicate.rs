use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::entity::EntityId;

pub fn codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::types::{self, def, record};
    record(vec![def("has_raid", types::bool(), types::b(false)), def("is_captain", types::bool(), types::b(false))])
}

#[derive(Debug)]
pub struct RaiderPredicate {
    has_raid: bool,
    is_captain: bool,
}

impl RaiderPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<RaiderPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let has_raid = record.with_default(tag, "has_raid", false, codec::bool_value);
            let is_captain = record.with_default(tag, "is_captain", false, codec::bool_value);
            record.build(|| Some(RaiderPredicate { has_raid, is_captain }))
        })
        .into_result()
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.mob(entity).and_then(|mob| mob.raider()).is_some() && level.has_raid(entity) == self.has_raid && level.is_captain(entity) == self.is_captain
    }
}
