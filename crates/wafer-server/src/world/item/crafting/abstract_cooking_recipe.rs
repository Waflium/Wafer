use std::ops::Deref;

use wafer_data::{Item, items};
use wafer_protocol::nbt::Nbt;

use super::cooking_book_category::CookingBookCategory;
use super::display::recipe_display::RecipeDisplay;
use super::ingredient::Ingredient;
use super::recipe::CommonInfo;
use super::display::slot_display::SlotDisplay;
use super::recipe_book_categories::RecipeBookCategory;
use super::recipe_type::RecipeType;
use super::single_item_recipe::SingleItemRecipe;
use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::item::component::values::ItemStackTemplate;

#[derive(Clone, Debug)]
pub struct CookingBookInfo {
    pub category: CookingBookCategory,
    pub group: String,
}

impl CookingBookInfo {
    pub fn decode(tag: &Nbt) -> DataResult<CookingBookInfo> {
        CookingBookInfo::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<CookingBookInfo> {
        let mut record = Record::default();
        let category = record.with_default(tag, "category", CookingBookCategory::Misc, |category| codec::string_enum(category, &CookingBookCategory::VALUES));
        let group = record.with_default(tag, "group", String::new(), |group| codec::string(group).map(str::to_string));
        record.build(|| Some(CookingBookInfo { category, group }))
    }
}

pub struct AbstractCookingRecipe {
    base: SingleItemRecipe,
    ty: RecipeType,
    book_info: CookingBookInfo,
    experience: f32,
    cooking_time: i32,
}

impl Deref for AbstractCookingRecipe {
    type Target = SingleItemRecipe;

    fn deref(&self) -> &SingleItemRecipe {
        &self.base
    }
}

impl AbstractCookingRecipe {
    pub fn decode(ty: RecipeType, tag: &Nbt) -> DataResult<AbstractCookingRecipe> {
        AbstractCookingRecipe::decode_partial(ty, tag).into_result()
    }

    fn decode_partial(ty: RecipeType, tag: &Nbt) -> DecodeResult<AbstractCookingRecipe> {
        let mut record = Record::default();
        let common_info = record.nested(|| CommonInfo::decode_partial(tag));
        let book_info = record.nested(|| CookingBookInfo::decode_partial(tag));
        let input = record.required(tag, "ingredient", Ingredient::decode);
        let result = record.required(tag, "result", ItemStackTemplate::decode_partial);
        let experience = record.with_default(tag, "experience", 0.0, codec::float);
        let cooking_time = record.required(tag, "cookingtime", codec::int);
        record.build(|| {
            let base = SingleItemRecipe::new(common_info?, input?, result?);
            Some(AbstractCookingRecipe { base, ty, book_info: book_info?, experience, cooking_time: cooking_time? })
        })
    }

    pub fn get_type(&self) -> RecipeType {
        self.ty
    }

    pub fn experience(&self) -> f32 {
        self.experience
    }

    pub fn cooking_time(&self) -> i32 {
        self.cooking_time
    }

    pub fn category(&self) -> CookingBookCategory {
        self.book_info.category
    }

    pub fn group(&self) -> &str {
        &self.book_info.group
    }

    fn furnace_icon(&self) -> Item {
        match self.ty {
            RecipeType::Blasting => items::BLAST_FURNACE,
            RecipeType::Smoking => items::SMOKER,
            RecipeType::CampfireCooking => items::CAMPFIRE,
            _ => items::FURNACE,
        }
    }

    pub fn display(&self) -> Vec<RecipeDisplay> {
        vec![RecipeDisplay::Furnace {
            ingredient: self.input().display(),
            fuel: SlotDisplay::AnyFuel,
            result: SlotDisplay::ItemStackSlotDisplay(self.result().clone()),
            crafting_station: SlotDisplay::ItemSlotDisplay(self.furnace_icon()),
            duration: self.cooking_time,
            experience: self.experience,
        }]
    }

    pub fn recipe_book_category(&self) -> RecipeBookCategory {
        match (self.ty, self.category()) {
            (RecipeType::Blasting, CookingBookCategory::Blocks) => RecipeBookCategory::BlastFurnaceBlocks,
            (RecipeType::Blasting, _) => RecipeBookCategory::BlastFurnaceMisc,
            (RecipeType::Smoking, _) => RecipeBookCategory::SmokerFood,
            (RecipeType::CampfireCooking, _) => RecipeBookCategory::Campfire,
            (_, CookingBookCategory::Blocks) => RecipeBookCategory::FurnaceBlocks,
            (_, CookingBookCategory::Food) => RecipeBookCategory::FurnaceFood,
            (_, CookingBookCategory::Misc) => RecipeBookCategory::FurnaceMisc,
        }
    }
}
