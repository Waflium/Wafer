use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::min_max_bounds::{Ints, ints_codec};
use crate::world::item::component::types::{self, Pair, def, record};
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::entity::EntityId;

pub fn codec() -> Pair {
    record(vec![def("size", ints_codec(), types::empty_map())])
}

#[derive(Debug)]
pub struct CubeMobPredicate {
    size: Ints,
}

impl CubeMobPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<CubeMobPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let size = record.with_default(tag, "size", Ints::ANY, Ints::decode_partial);
            record.build(|| Some(CubeMobPredicate { size }))
        })
        .into_result()
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        level.mob(entity).and_then(|mob| mob.cube_mob_size()).is_some_and(|size| self.size.matches(size))
    }
}
