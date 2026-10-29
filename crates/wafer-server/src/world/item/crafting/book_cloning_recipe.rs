use wafer_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use super::transmute_recipe::create_with_original_components_extra;
use crate::advancements::predicates::min_max_bounds::{Bounds, Ints};
use crate::util::codec::{DataResult, DecodeResult, Record};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::item::component::values::ItemStackTemplate;

const DEFAULT_BOOK_GENERATION_RANGES: Ints = Ints { bounds: Bounds { min: Some(0), max: Some(1) } };

pub struct BookCloningRecipe {
    source: Ingredient,
    material: Ingredient,
    allowed_generations: Ints,
    result: ItemStackTemplate,
}

fn decode_allowed_generations(tag: &Nbt) -> DecodeResult<Ints> {
    Ints::decode_partial(tag).validate(|bounds| match (bounds.bounds.min, bounds.bounds.max) {
        (Some(min), Some(max)) if min >= 0 && max <= 2 => Ok(()),
        _ => Err(format!("Range must be within [0..2], but was {}", bounds.bounds.as_range_string())),
    })
}

fn generation(content: &Nbt) -> i32 {
    content.get("generation").and_then(Nbt::as_i64).unwrap_or(0) as i32
}

fn craft_copy(content: &Nbt) -> Nbt {
    let Nbt::Compound(fields) = content else { return content.clone() };
    let mut fields: Vec<(String, Nbt)> = fields.iter().filter(|(key, _)| key != "generation").cloned().collect();
    fields.push(("generation".into(), Nbt::Int(generation(content) + 1)));
    Nbt::Compound(fields)
}

impl BookCloningRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<BookCloningRecipe> {
        BookCloningRecipe::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<BookCloningRecipe> {
        let mut record = Record::default();
        let source = record.required(tag, "source", Ingredient::decode);
        let material = record.required(tag, "material", Ingredient::decode);
        let allowed_generations = record.with_default(tag, "allowed_generations", DEFAULT_BOOK_GENERATION_RANGES, decode_allowed_generations);
        let result = record.required(tag, "result", ItemStackTemplate::decode_partial);
        record.build(|| Some(BookCloningRecipe { source: source?, material: material?, allowed_generations, result: result? }))
    }

    fn can_craft_copy(&self, written_book_content: &Nbt) -> bool {
        self.allowed_generations.matches(generation(written_book_content))
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() < 2 {
            return false;
        }
        let mut has_material = false;
        let mut has_source = false;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.source.test(item_stack) {
                if !item_stack.get(dc::WRITTEN_BOOK_CONTENT).is_some_and(|content| self.can_craft_copy(content)) {
                    return false;
                }
                if has_source {
                    return false;
                }
                has_source = true;
            } else {
                if !self.material.test(item_stack) {
                    return false;
                }
                has_material = true;
            }
        }
        has_source && has_material
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let mut count = 0;
        let mut source: Option<&ItemStack> = None;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.source.test(item_stack) && item_stack.has(dc::WRITTEN_BOOK_CONTENT) {
                if source.is_some() {
                    return ItemStack::empty();
                }
                source = Some(item_stack);
            } else {
                if !self.material.test(item_stack) {
                    return ItemStack::empty();
                }
                count += 1;
            }
        }
        let Some(source) = source else { return ItemStack::empty() };
        let Some(source_content) = source.get(dc::WRITTEN_BOOK_CONTENT) else { return ItemStack::empty() };
        let copied_content = craft_copy(source_content);
        let mut result = create_with_original_components_extra(&self.result, source, count - 1);
        result.set_value(dc::WRITTEN_BOOK_CONTENT, copied_content);
        result
    }

    pub fn get_remaining_items(&self, input: &CraftingInput) -> Vec<ItemStack> {
        let mut result = vec![ItemStack::empty(); input.size()];
        for (slot, remaining) in result.iter_mut().enumerate() {
            let item_stack = input.get_item(slot);
            if let Some(remainder) = item_stack.get_item().crafting_remainder() {
                *remaining = ItemStack::new(remainder, 1);
            } else if item_stack.has(dc::WRITTEN_BOOK_CONTENT) {
                *remaining = item_stack.copy_with_count(1);
                break;
            }
        }
        result
    }
}
