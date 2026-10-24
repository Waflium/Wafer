use wafer_protocol::nbt::Nbt;

use super::crafting_recipe::CraftingBookInfo;
use super::placement_info::PlacementInfo;
use super::recipe::CommonInfo;
use crate::util::codec::{DecodeResult, Record};

pub struct NormalCraftingRecipe {
    pub common_info: CommonInfo,
    pub book_info: CraftingBookInfo,
    pub placement_info: PlacementInfo,
}

impl NormalCraftingRecipe {
    pub fn decode_partial(tag: &Nbt) -> DecodeResult<NormalCraftingRecipe> {
        let mut record = Record::default();
        let common_info = record.absorb(CommonInfo::decode_partial(tag));
        let book_info = record.absorb(CraftingBookInfo::decode_partial(tag));
        record.build(|| Some(NormalCraftingRecipe { common_info: common_info?, book_info: book_info?, placement_info: PlacementInfo::NOT_PLACEABLE }))
    }
}
