use wafer_protocol::nbt::Nbt;

use super::crafting_input::CraftingInput;
use super::ingredient::Ingredient;
use crate::util::codec::{DataResult, DecodeResult, Record};
use crate::world::item::ItemStack;
use crate::world::item::component::values::ItemStackTemplate;
use crate::world::item::component::{self as dc, DataComponentPatch};

pub struct FireworkRocketRecipe {
    shell: Ingredient,
    fuel: Ingredient,
    star: Ingredient,
    result: ItemStackTemplate,
}

impl FireworkRocketRecipe {
    pub fn decode(tag: &Nbt) -> DataResult<FireworkRocketRecipe> {
        FireworkRocketRecipe::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<FireworkRocketRecipe> {
        let mut record = Record::default();
        let shell = record.required(tag, "shell", Ingredient::decode);
        let fuel = record.required(tag, "fuel", Ingredient::decode);
        let star = record.required(tag, "star", Ingredient::decode);
        let result = record.required(tag, "result", ItemStackTemplate::decode_partial);
        record.build(|| Some(FireworkRocketRecipe { shell: shell?, fuel: fuel?, star: star?, result: result? }))
    }

    pub fn matches(&self, input: &CraftingInput) -> bool {
        if input.ingredient_count() < 2 {
            return false;
        }
        let mut has_shell = false;
        let mut fuel_count = 0;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.shell.test(item_stack) {
                if has_shell {
                    return false;
                }
                has_shell = true;
            } else if self.fuel.test(item_stack) {
                fuel_count += 1;
                if fuel_count > 3 {
                    return false;
                }
            } else if !self.star.test(item_stack) {
                return false;
            }
        }
        has_shell && fuel_count >= 1
    }

    pub fn assemble(&self, input: &CraftingInput) -> ItemStack {
        let mut explosions = Vec::new();
        let mut fuel_count = 0;
        for slot in 0..input.size() {
            let item_stack = input.get_item(slot);
            if item_stack.is_empty() {
                continue;
            }
            if self.fuel.test(item_stack) {
                fuel_count += 1;
            } else if self.star.test(item_stack)
                && let Some(explosion) = item_stack.get(dc::FIREWORK_EXPLOSION)
            {
                explosions.push(explosion.clone());
            }
        }
        let fireworks = Nbt::Compound(vec![("flight_duration".into(), Nbt::Byte(fuel_count as i8)), ("explosions".into(), Nbt::List(explosions))]);
        let mut components = DataComponentPatch::default();
        components.put(dc::FIREWORKS, Some(dc::FIREWORKS.decode(&fireworks).unwrap_or(fireworks)));
        self.result.apply(self.result.count, &components)
    }
}
