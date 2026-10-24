use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::ingredient::Ingredient;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct ShapelessRecipe {
    pub base: NormalCraftingRecipe,
    result: ItemStackTemplate,
    ingredients: Vec<Ingredient>,
}

impl ShapelessRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<ShapelessRecipe> {
        ShapelessRecipe::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<ShapelessRecipe> {
        let mut record = Record::default();
        let base = record.absorb(NormalCraftingRecipe::decode_partial(tag));
        let result = record.required(tag, "result", ItemStackTemplate::decode_partial);
        let ingredients = record.required(tag, "ingredients", |ingredients| codec::list_between_partial(ingredients, 1, 9, |element| DecodeResult::from(Ingredient::decode(element))));
        record.build(|| {
            let mut base = base?;
            let ingredients = ingredients?;
            base.placement_info = PlacementInfo::create_list(ingredients.clone());
            Some(ShapelessRecipe { base, result: result?, ingredients })
        })
    }

    pub fn matches(&self, input: &mut CraftingInput) -> bool {
        if input.ingredient_count() != self.ingredients.len() {
            return false;
        }
        if input.size() == 1 && self.ingredients.len() == 1 {
            return self.ingredients[0].test(input.get_item(0));
        }
        let placement_info = &self.base.placement_info;
        !placement_info.is_impossible_to_place() && input.stacked_contents().can_craft_ingredients(placement_info.ingredients(), None)
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.create().unwrap_or_else(ItemStack::empty)
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        vec![RecipeDisplay::ShapelessCrafting {
            ingredients: self.ingredients.iter().map(Ingredient::display).collect(),
            result: SlotDisplay::ItemStackSlotDisplay(self.result.clone()),
            crafting_station: SlotDisplay::ItemSlotDisplay(items::CRAFTING_TABLE),
        }]
    }
}
