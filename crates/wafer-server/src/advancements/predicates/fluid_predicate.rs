use wafer_core::BlockPos;
use wafer_protocol::nbt::Nbt;

use super::state_properties_predicate::{self, StatePropertiesPredicate};
use crate::core::holder_set::HolderSet;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::types::{self, Pair, opt, record};
use crate::world::level::BlockGetter;

pub fn codec() -> Pair {
    record(vec![opt("fluids", types::holder_set("minecraft:fluid")), opt("state", state_properties_predicate::codec())])
}

#[derive(Debug)]
pub struct FluidPredicate {
    fluids: Option<HolderSet>,
    properties: Option<StatePropertiesPredicate>,
}

impl FluidPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<FluidPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let fluids = record.optional(tag, "fluids", |t| HolderSet::decode_partial("minecraft:fluid", t));
            let properties = record.optional(tag, "state", StatePropertiesPredicate::decode);
            record.build(|| Some(FluidPredicate { fluids, properties }))
        })
        .into_result()
    }

    pub fn matches(&self, level: &mut ServerLevel, pos: BlockPos) -> bool {
        if !level.is_loaded(pos) {
            return false;
        }
        let state = level.fluid_state(pos);
        if self.fluids.as_ref().is_some_and(|fluids| !fluids.contains(state.fluid as i32)) {
            return false;
        }
        self.properties.as_ref().is_none_or(|properties| properties.matches_fluid(state))
    }
}
