use wafer_protocol::nbt::Nbt;

use super::ItemStack;
use super::component::{self as dc};
use super::crafting::recipe_holder::RecipeHolder;
use super::crafting::recipe_manager::RecipeManager;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::{InteractionHand, InteractionResult};

pub fn use_item(level: &mut ServerLevel, player: EntityId, hand: InteractionHand) -> (InteractionResult, Option<ItemStack>) {
    let mut item_stack = level.player(player).item_in_hand(hand).unwrap_or_else(ItemStack::empty);
    let recipe_ids: Vec<String> = item_stack
        .get(dc::RECIPES)
        .map_or_else(Vec::new, |ids| ids.as_list().iter().filter_map(Nbt::as_str).filter_map(crate::commands::arguments::parse_identifier).collect());
    let item = item_stack.item;
    item_stack.consume(1, level, Some(player));
    level.player_mut(player).set_item_in_hand(hand, Some(item_stack));
    if recipe_ids.is_empty() {
        return (InteractionResult::Fail, None);
    }
    let recipe_manager = RecipeManager::get();
    let mut recipes: Vec<&'static RecipeHolder> = Vec::with_capacity(recipe_ids.len());
    for recipe_id in &recipe_ids {
        let Some(recipe) = recipe_manager.by_key(recipe_id) else {
            error!("Invalid recipe: ResourceKey[minecraft:recipe / {recipe_id}]");
            return (InteractionResult::Fail, None);
        };
        recipes.push(recipe);
    }
    level.award_recipes(player, &recipes);
    level.award_stat(player, crate::stats::stats::ITEM_USED.get(item.0), 1);
    (InteractionResult::SUCCESS, None)
}
