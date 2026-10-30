use wafer_protocol::nbt::Nbt;

use super::crafting_recipe::CraftingBookInfo;
use super::placement_info::PlacementInfo;
use super::recipe::CommonInfo;
use crate::util::codec::Record;

pub struct NormalCraftingRecipe {
    pub common_info: CommonInfo,
    pub book_info: CraftingBookInfo,
    pub placement_info: PlacementInfo,
}

impl NormalCraftingRecipe {
    pub fn decode_in(record: &mut Record, tag: &Nbt) -> Option<NormalCraftingRecipe> {
        let common_info = record.nested(|| CommonInfo::decode_partial(tag));
        let book_info = record.nested(|| CraftingBookInfo::decode_partial(tag));
        Some(NormalCraftingRecipe { common_info: common_info?, book_info: book_info?, placement_info: PlacementInfo::NOT_PLACEABLE })
    }
}
