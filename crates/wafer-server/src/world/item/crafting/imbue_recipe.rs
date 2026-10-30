use wafer_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::display::recipe_display::RecipeDisplay;
use super::display::slot_display::SlotDisplay;
use super::ingredient::Ingredient;
use super::normal_crafting_recipe::NormalCraftingRecipe;
use super::placement_info::PlacementInfo;
use crate::util::codec::{DataResult, DecodeResult, Record};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::values::ItemStackTemplate;

pub struct ImbueRecipe {
    pub base: NormalCraftingRecipe,
    source: Ingredient,
    material: Ingredient,
    result: ItemStackTemplate,
}

impl ImbueRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<ImbueRecipe> {
        ImbueRecipe::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<ImbueRecipe> {
        let mut record = Record::default();
        let base = NormalCraftingRecipe::decode_in(&mut record, tag);
        let source = record.required(tag, "source", Ingredient::decode);
        let material = record.required(tag, "material", Ingredient::decode);
        let result = record.required(tag, "result", ItemStackTemplate::decode_partial);
        record.build(|| {
            let mut base = base?;
            let (source, material) = (source?, material?);
            let m = || material.clone();
            base.placement_info = PlacementInfo::create_list(vec![m(), m(), m(), m(), source.clone(), m(), m(), m(), m()]);
            Some(ImbueRecipe { base, source, material, result: result? })
        })
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.width() != 3 || input.height() != 3 || input.ingredient_count() != 9 {
            return false;
        }
        for y in 0..input.height() {
            for x in 0..input.width() {
                let item_stack = input.get_item_at(x, y);
                if item_stack.is_empty() {
                    return false;
                }
                let ingredient = if x == 1 && y == 1 { &self.source } else { &self.material };
                if !ingredient.test(item_stack) {
                    return false;
                }
            }
        }
        true
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let source = input.get_item_at(1, 1);
        let mut result = self.result.create().unwrap_or_else(ItemStack::empty);
        result.set(dc::POTION_CONTENTS, source.get(dc::POTION_CONTENTS).cloned());
        result
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        let material = self.material.display();
        let source = SlotDisplay::WithAnyPotion(Box::new(self.source.display()));
        let m = || material.clone();
        vec![RecipeDisplay::ShapedCrafting {
            width: 3,
            height: 3,
            ingredients: vec![m(), m(), m(), m(), source, m(), m(), m(), m()],
            result: SlotDisplay::WithAnyPotion(Box::new(SlotDisplay::ItemStackSlotDisplay(self.result.clone()))),
            crafting_station: SlotDisplay::ItemSlotDisplay(wafer_data::items::CRAFTING_TABLE),
        }]
    }
}
