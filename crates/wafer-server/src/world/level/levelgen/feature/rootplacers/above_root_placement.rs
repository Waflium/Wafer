use serde_json::Value;

use crate::util::valueproviders::float;
use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::level::levelgen::feature::stateproviders::block_state_provider::{self, BlockStateProviderHolder};

pub struct AboveRootPlacement {
    pub above_root_provider: BlockStateProviderHolder,
    pub above_root_placement_chance: f32,
}

pub fn codec() -> Pair {
    record(vec![
        req("above_root_provider", block_state_provider::holder_codec()),
        req("above_root_placement_chance", types::float(types::codec_float_range(0.0, 1.0))),
    ])
}

impl AboveRootPlacement {
    pub fn parse(json: &Value) -> Result<AboveRootPlacement, String> {
        Ok(AboveRootPlacement {
            above_root_provider: BlockStateProviderHolder::parse(json.get("above_root_provider").ok_or("missing above_root_provider")?)?,
            above_root_placement_chance: float(json.get("above_root_placement_chance").ok_or("missing above_root_placement_chance")?)?,
        })
    }
}
