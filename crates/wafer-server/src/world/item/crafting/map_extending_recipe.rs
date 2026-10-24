use wafer_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use super::shaped_recipe_pattern::ShapedRecipePattern;
use super::transmute_recipe::create_with_original_components;
use super::transmute_result::TransmuteResult;
use crate::server::level::ServerLevel;
use crate::util::codec::{DataResult, DecodeResult, Record};
use crate::world::item::component as dc;
use crate::world::item::component::map_post_processing::MapPostProcessing;
use crate::world::item::{ItemStack, map_item};
use crate::world::level::saveddata::maps::map_item_saved_data::MAX_SCALE;

pub struct MapExtendingRecipe {
    pattern: ShapedRecipePattern,
    result: TransmuteResult,
}

fn find_filled_map(input: &CraftingInput) -> Option<&ItemStack> {
    (0..input.size()).map(|i| input.get_item(i)).find(|item_stack| item_stack.has(dc::MAP_ID))
}

impl MapExtendingRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<MapExtendingRecipe> {
        MapExtendingRecipe::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<MapExtendingRecipe> {
        let mut record = Record::default();
        let map = record.required(tag, "map", Ingredient::decode);
        let material = record.required(tag, "material", Ingredient::decode);
        let result = record.required(tag, "result", TransmuteResult::decode_partial);
        record.build(|| {
            let pattern = ShapedRecipePattern::of(&[('#', material?), ('x', map?)], &["###", "#x#", "###"]);
            Some(MapExtendingRecipe { pattern, result: result? })
        })
    }

    pub fn matches(&self, input: &CraftingInput, level: &mut ServerLevel) -> bool {
        if !self.pattern.matches(input) {
            return false;
        }
        let Some(map) = find_filled_map(input) else { return false };
        map_item::get_saved_data(map, level).is_some_and(|data| data.borrow().scale < MAX_SCALE)
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let empty = ItemStack::empty();
        let source_map = find_filled_map(input).unwrap_or(&empty);
        let mut map = create_with_original_components(&self.result.resolve(source_map.get_item()), source_map);
        map.set(dc::MAP_POST_PROCESSING, Some(MapPostProcessing::Scale.to_nbt()));
        map
    }
}
