use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use super::min_max_bounds::{Ints, ints_codec};
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::types::{self, Pair, def, record};

pub fn codec() -> Pair {
    record(vec![def("light", ints_codec(), types::empty_map())])
}

#[derive(Debug)]
pub struct LightPredicate {
    composite: Ints,
}

impl LightPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<LightPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let composite = record.with_default(tag, "light", Ints::ANY, Ints::decode_partial);
            record.build(|| Some(LightPredicate { composite }))
        })
        .into_result()
    }

    pub fn matches(&self, level: &ServerLevel, pos: BlockPos) -> bool {
        level.is_loaded(pos) && self.composite.matches(level.max_local_raw_brightness(pos, level.sky_darken()))
    }
}
