use wafer_protocol::nbt::Nbt;

use super::brewing_input::BrewingInput;
use super::potion_ingredient::PotionIngredient;
use crate::util::codec::{DataResult, DecodeResult, Record};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct BrewingRecipe {
    input: PotionIngredient,
    reagent: PotionIngredient,
    output: ItemStackTemplate,
}

impl BrewingRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<BrewingRecipe> {
        BrewingRecipe::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<BrewingRecipe> {
        let mut record = Record::default();
        let input = record.required(tag, "input", PotionIngredient::decode);
        let reagent = record.required(tag, "reagent", PotionIngredient::decode);
        let output = record.required(tag, "output", ItemStackTemplate::decode_partial);
        record.build(|| Some(BrewingRecipe { input: input?, reagent: reagent?, output: output? }))
    }

    pub fn get_input(&self) -> &PotionIngredient {
        &self.input
    }

    pub fn get_reagent(&self) -> &PotionIngredient {
        &self.reagent
    }

    pub fn matches(&self, brewing_input: &BrewingInput) -> bool {
        self.input.test(brewing_input.input) && self.reagent.test(brewing_input.reagent)
    }

    pub fn assemble(&self) -> ItemStack {
        self.output.create().unwrap_or_else(ItemStack::empty)
    }
}
