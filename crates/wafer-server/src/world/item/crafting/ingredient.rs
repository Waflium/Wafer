use wafer_data::Item;
use wafer_protocol::nbt::Nbt;

use super::display::slot_display::SlotDisplay;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{DataResult, DecodeResult};
use crate::world::entity::player::stacked_contents::IngredientInfo;
use crate::world::item::ItemStack;
use crate::world::item::trading::item_cost::decode_item;

#[derive(Clone, Debug, PartialEq)]
pub struct Ingredient {
    values: HolderSet,
}

impl Ingredient {
    pub fn decode(tag: &Nbt) -> DataResult<Ingredient> {
        Ingredient::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<Ingredient> {
        HolderSet::decode_partial_with("minecraft:item", tag, |element| DecodeResult::from(decode_item(element)).map(|item| item.0 as i32))
            .validate(|values| match values {
                HolderSet::Direct(direct_values) if direct_values.is_empty() => Err("List must have contents".to_string()),
                _ => Ok(()),
            })
            .map(|values| Ingredient { values })
    }

    pub fn test_optional_ingredient(ingredient: Option<&Ingredient>, stack: &ItemStack) -> bool {
        match ingredient {
            Some(value) => value.test(stack),
            None => stack.is_empty(),
        }
    }

    pub fn items(&self) -> impl Iterator<Item = Item> + '_ {
        self.values.contents().iter().map(|&id| Item(id as u16))
    }

    pub fn is_empty(&self) -> bool {
        self.values.contents().is_empty()
    }

    pub fn test(&self, input: &ItemStack) -> bool {
        self.values.contains(input.get_item().0 as i32)
    }

    pub fn accepts_item(&self, item: Item) -> bool {
        self.values.contains(item.0 as i32)
    }

    pub fn of(item: Item) -> Ingredient {
        Ingredient { values: HolderSet::Direct(vec![item.0 as i32]) }
    }

    pub fn values(&self) -> &HolderSet {
        &self.values
    }

    pub fn display(&self) -> SlotDisplay {
        SlotDisplay::TagSlotDisplay(self.values.clone())
    }

    pub fn optional_ingredient_to_display(ingredient: Option<&Ingredient>) -> SlotDisplay {
        ingredient.map_or(SlotDisplay::Empty, Ingredient::display)
    }

    pub fn get_single_item(&self) -> Option<Item> {
        match self.values.contents() {
            [single] => Some(Item(*single as u16)),
            _ => None,
        }
    }
}

impl IngredientInfo<Item> for Ingredient {
    fn accepts_item(&self, item: Item) -> bool {
        Ingredient::accepts_item(self, item)
    }
}
