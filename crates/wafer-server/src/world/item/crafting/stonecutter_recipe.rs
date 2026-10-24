use std::ops::Deref;

use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::single_item_recipe::SingleItemRecipe;
use crate::util::codec::DataResult;

pub struct StonecutterRecipe {
    base: SingleItemRecipe,
}

impl Deref for StonecutterRecipe {
    type Target = SingleItemRecipe;

    fn deref(&self) -> &SingleItemRecipe {
        &self.base
    }
}

impl StonecutterRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<StonecutterRecipe> {
        SingleItemRecipe::decode_partial(tag).map(|base| StonecutterRecipe { base }).into_result()
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        vec![RecipeDisplay::Stonecutter { input: self.input().display(), result: self.result_display(), crafting_station: SlotDisplay::ItemSlotDisplay(items::STONECUTTER) }]
    }

    pub fn result_display(&self) -> SlotDisplay {
        SlotDisplay::ItemStackSlotDisplay(self.result().clone())
    }
}
