use wafer_protocol::nbt::Nbt;

use super::placement_info::PlacementInfo;
use super::recipe::CommonInfo;
use crate::util::codec::DecodeResult;

pub struct SimpleSmithingRecipe {
    pub common_info: CommonInfo,
    pub placement_info: PlacementInfo,
}

impl SimpleSmithingRecipe {
    pub fn decode_partial(tag: &Nbt) -> DecodeResult<SimpleSmithingRecipe> {
        CommonInfo::decode_partial(tag).map(|common_info| SimpleSmithingRecipe { common_info, placement_info: PlacementInfo::NOT_PLACEABLE })
    }
}
