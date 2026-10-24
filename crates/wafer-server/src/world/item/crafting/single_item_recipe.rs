use wafer_protocol::nbt::Nbt;

use super::ingredient::Ingredient;
use super::placement_info::PlacementInfo;
use super::recipe::CommonInfo;
use super::single_recipe_input::SingleRecipeInput;
use crate::util::codec::{DataResult, DecodeResult, Record};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;

pub struct SingleItemRecipe {
    pub common_info: CommonInfo,
    input: Ingredient,
    result: ItemStackTemplate,
    placement_info: PlacementInfo,
}

impl SingleItemRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<SingleItemRecipe> {
        SingleItemRecipe::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<SingleItemRecipe> {
        let mut record = Record::default();
        let common_info = record.absorb(CommonInfo::decode_partial(tag));
        let input = record.required(tag, "ingredient", Ingredient::decode);
        let result = record.required(tag, "result", ItemStackTemplate::decode_partial);
        record.build(|| Some(SingleItemRecipe::new(common_info?, input?, result?)))
    }

    pub fn new(common_info: CommonInfo, input: Ingredient, result: ItemStackTemplate) -> SingleItemRecipe {
        let placement_info = PlacementInfo::create(&input);
        SingleItemRecipe { common_info, input, result, placement_info }
    }

    pub fn matches(&self, input: &SingleRecipeInput) -> bool {
        self.input.test(input.item)
    }

    pub fn show_notification(&self) -> bool {
        self.common_info.show_notification
    }

    pub fn input(&self) -> &Ingredient {
        &self.input
    }

    pub fn result(&self) -> &ItemStackTemplate {
        &self.result
    }

    pub fn placement_info(&self) -> &PlacementInfo {
        &self.placement_info
    }

    pub fn assemble(&self) -> ItemStack {
        self.result.create().unwrap_or_else(ItemStack::empty)
    }
}
