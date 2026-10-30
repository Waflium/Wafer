use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::server::reloadable_server_registries;
use crate::util::codec::{self, DecodeResult};
use crate::world::entity::EntityId;
use crate::world::item::crafting::recipe::Recipe;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct RecipeUnlockedTrigger(pub CriterionTrigger);

impl RecipeUnlockedTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, recipe: &str) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::RecipeUnlocked(t) = instance else { return false };
            t.matches(recipe)
        });
    }
}

pub fn decode_recipe_list(tag: &Nbt) -> DecodeResult<Vec<Option<String>>> {
    codec::holder_set_partial(tag, reloadable_server_registries::RECIPE, |element| {
        let Ok(key) = codec::identifier(element) else {
            return DecodeResult::from(Recipe::decode(element).map(|_| None));
        };
        if reloadable_server_registries::contains_key(reloadable_server_registries::RECIPE, &key) {
            crate::core::data_registries::known_reference(reloadable_server_registries::RECIPE, &key);
            DecodeResult::success(Some(key))
        } else if reloadable_server_registries::is_loading() {
            reloadable_server_registries::record_unbound(reloadable_server_registries::RECIPE, &key);
            DecodeResult::success(Some(key))
        } else {
            DecodeResult::error(format!("Missing element ResourceKey[{} / {key}]", reloadable_server_registries::RECIPE))
        }
    })
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    recipes: Vec<Option<String>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let recipes = fields.required("recipes", decode_recipe_list);
            Some(TriggerInstance { player, recipes: recipes? })
        })
    }

    pub fn matches(&self, recipe: &str) -> bool {
        self.recipes.iter().any(|holder| holder.as_deref() == Some(recipe))
    }
}
