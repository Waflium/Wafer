use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::ingredient::Ingredient;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use super::shaped_recipe_pattern::ShapedRecipePattern;
use crate::util::codec::{DataResult, DecodeResult, Record};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct ShapedRecipe {
    pub base: NormalCraftingRecipe,
    pattern: ShapedRecipePattern,
    result: ItemStackTemplate,
}

impl ShapedRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<ShapedRecipe> {
        ShapedRecipe::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<ShapedRecipe> {
        let mut record = Record::default();
        let base = NormalCraftingRecipe::decode_in(&mut record, tag);
        let pattern = record.nested(|| ShapedRecipePattern::decode_partial(tag));
        let result = record.required(tag, "result", ItemStackTemplate::decode_partial);
        record.build(|| {
            let mut base = base?;
            let pattern = pattern?;
            base.placement_info = PlacementInfo::create_from_optionals(pattern.ingredients());
            Some(ShapedRecipe { base, pattern, result: result? })
        })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        self.pattern.matches(input)
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.create().unwrap_or_else(ItemStack::empty)
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        vec![RecipeDisplay::ShapedCrafting {
            width: self.pattern.width() as i32,
            height: self.pattern.height() as i32,
            ingredients: self.pattern.ingredients().iter().map(|ingredient| Ingredient::optional_ingredient_to_display(ingredient.as_ref())).collect(),
            result: SlotDisplay::ItemStackSlotDisplay(self.result.clone()),
            crafting_station: SlotDisplay::ItemSlotDisplay(items::CRAFTING_TABLE),
        }]
    }

    pub fn get_width(&self) -> usize {
        self.pattern.width()
    }

    pub fn get_height(&self) -> usize {
        self.pattern.height()
    }
}
