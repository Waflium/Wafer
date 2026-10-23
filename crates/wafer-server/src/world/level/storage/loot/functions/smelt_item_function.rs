use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::item::crafting::recipe_holder::RecipeHolder;
use crate::world::item::crafting::recipe_manager::RecipeManager;
use crate::world::item::crafting::recipe_type::RecipeType;
use crate::world::item::crafting::single_recipe_input::SingleRecipeInput;
use crate::world::level::storage::loot::loot_context::LootContext;

pub struct SmeltItemFunction {
    use_input_count: bool,
}

impl SmeltItemFunction {
    pub fn decode(tag: &Nbt) -> DecodeResult<SmeltItemFunction> {
        let mut record = codec::Record::default();
        let use_input_count = record.with_default(tag, "use_input_count", true, codec::bool_value);
        record.build(|| Some(SmeltItemFunction { use_input_count }))
    }

    pub fn run(&self, item_stack: ItemStack, _context: &mut LootContext) -> ItemStack {
        if item_stack.is_empty() {
            return item_stack;
        }
        let recipe = RecipeManager::get().get_cooking_recipe_for(RecipeType::Smelting, &SingleRecipeInput { item: &item_stack }, None).and_then(RecipeHolder::cooking);
        if let Some(recipe) = recipe {
            let result = recipe.assemble();
            if !result.is_empty() {
                let new_count = if self.use_input_count { item_stack.count } else { 1 } * result.count;
                return result.copy_with_count(new_count.min(result.max_stack_size()));
            }
        }
        warn!("Couldn't smelt {} {} because there is no smelting recipe", item_stack.count, item_stack.item.name());
        item_stack
    }
}
