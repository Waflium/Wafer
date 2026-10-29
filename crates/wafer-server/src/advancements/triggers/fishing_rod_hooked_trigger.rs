use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validation_context_source::ValidationContextSource;
use crate::world::level::storage::loot::validatable;

#[derive(Clone, Copy)]
pub struct FishingRodHookedTrigger(pub CriterionTrigger);

impl FishingRodHookedTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, rod: &ItemStack, hooked_in: EntityId, items: &[ItemStack]) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::FishingRodHooked(t) = instance else { return false };
            t.matches(level, player, rod, hooked_in, items)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    rod: Option<ItemPredicate>,
    entity: Option<Holder<LootItemCondition>>,
    item: Option<ItemPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let rod = fields.optional("rod", ItemPredicate::decode);
            let entity = fields.condition("entity");
            let item = fields.optional("item", ItemPredicate::decode);
            Some(TriggerInstance { player, rod, entity, item })
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, rod: &ItemStack, hooked_in: EntityId, items: &[ItemStack]) -> bool {
        if self.rod.as_ref().is_some_and(|predicate| !predicate.test(rod)) {
            return false;
        }
        if !simple_criterion_trigger::matches_entity(level, player, self.entity.as_ref(), hooked_in) {
            return false;
        }
        if let Some(predicate) = &self.item {
            let mut matched = false;
            if let Some(LevelEntity::Item(item)) = level.find_entity(hooked_in)
                && predicate.test(&item.item)
            {
                matched = true;
            }
            if items.iter().any(|item| predicate.test(item)) {
                matched = true;
            }
            if !matched {
                return false;
            }
        }
        true
    }
}

impl TriggerInstance {
    pub fn validate(&self, validator: &mut ValidationContextSource) {
        simple_criterion_trigger::validate(validator, self.player.as_ref());
        validatable::validate_optional_holder(validator.entity_context(), "entity", self.entity.as_ref());
    }
}
