use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::recipe_unlocked_trigger::decode_recipe_list;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct RecipeCraftedTrigger(pub CriterionTrigger);

impl RecipeCraftedTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, id: &str, used_ingredients: &[ItemStack]) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::RecipeCrafted(t) = instance else { return false };
            t.matches(id, used_ingredients)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    recipes: Vec<Option<String>>,
    ingredients: Vec<ItemPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let recipes = fields.required("recipes", decode_recipe_list);
            let ingredients = fields.list("ingredients", ItemPredicate::decode);
            Some(TriggerInstance { player, recipes: recipes?, ingredients })
        })
    }

    fn matches(&self, id: &str, used_ingredients: &[ItemStack]) -> bool {
        if !self.recipes.iter().any(|holder| holder.as_deref() == Some(id)) {
            return false;
        }
        let mut remaining: Vec<&ItemStack> = used_ingredients.iter().collect();
        for predicate in &self.ingredients {
            match remaining.iter().position(|item| predicate.test(item)) {
                Some(found) => {
                    remaining.remove(found);
                }
                None => return false,
            }
        }
        true
    }
}
