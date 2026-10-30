use wafer_protocol::nbt::Nbt;

use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::entity::EntityId;

pub fn codec() -> crate::world::item::component::types::Pair {
    use crate::world::item::component::types::{self, opt, record};
    record(vec![opt("in_open_water", types::bool())])
}

#[derive(Debug)]
pub struct FishingHookPredicate {
    in_open_water: Option<bool>,
}

impl FishingHookPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FishingHookPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let in_open_water = record.optional(tag, "in_open_water", codec::bool_value);
            record.build(|| Some(FishingHookPredicate { in_open_water }))
        })
        .into_result()
    }

    pub fn matches(&self, level: &ServerLevel, entity: EntityId) -> bool {
        let Some(in_open_water) = self.in_open_water else { return true };
        level.fishing_hook_open_water(entity).is_some_and(|open_water| open_water == in_open_water)
    }
}
