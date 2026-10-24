use wafer_data::item_tags;
use wafer_protocol::nbt::Nbt;

use super::ingredient::Ingredient;
use super::recipe_manager::RecipeManager;
use super::recipe_property_set::BREWING_INPUTS;
use crate::core::component::predicates::potions_predicate::PotionsPredicate;
use crate::util::codec::{DataResult, DecodeResult, Record};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

pub struct PotionIngredient {
    pub ingredient: Ingredient,
    potions: Option<PotionsPredicate>,
}

impl PotionIngredient {
    pub fn decode(tag: &Nbt) -> DataResult<PotionIngredient> {
        PotionIngredient::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<PotionIngredient> {
        let mut record = Record::default();
        let ingredient = record.required(tag, "item", Ingredient::decode);
        let potions = record.optional(tag, "potion_contents", PotionsPredicate::decode);
        record.build(|| Some(PotionIngredient { ingredient: ingredient?, potions }))
    }

    pub fn test(&self, input: &ItemStack) -> bool {
        if !self.ingredient.test(input) {
            return false;
        }
        self.potions.as_ref().is_none_or(|potions| input.get(dc::POTION_CONTENTS).is_some_and(|value| potions.matches(value)))
    }

    pub fn is_potion_input(item_stack: &ItemStack) -> bool {
        RecipeManager::get().property_set(BREWING_INPUTS).test(item_stack) || item_stack.get_item().is(&item_tags::BREWING_POTION_INPUTS)
    }
}
