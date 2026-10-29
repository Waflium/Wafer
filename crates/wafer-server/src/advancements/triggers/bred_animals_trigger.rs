use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validation_context_source::ValidationContextSource;
use crate::world::level::storage::loot::validatable;

#[derive(Clone, Copy)]
pub struct BredAnimalsTrigger(pub CriterionTrigger);

impl BredAnimalsTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, parent: EntityId, partner: EntityId, child: Option<EntityId>) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::BredAnimals(t) = instance else { return false };
            t.matches(level, player, parent, partner, child)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    parent: Option<Holder<LootItemCondition>>,
    partner: Option<Holder<LootItemCondition>>,
    child: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let parent = fields.condition("parent");
            let partner = fields.condition("partner");
            let child = fields.condition("child");
            Some(TriggerInstance { player, parent, partner, child })
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, parent: EntityId, partner: EntityId, child: Option<EntityId>) -> bool {
        if self.child.is_some() {
            let Some(child) = child else { return false };
            if !simple_criterion_trigger::matches_entity(level, player, self.child.as_ref(), child) {
                return false;
            }
        }
        let matches = |level: &mut ServerLevel, predicate: &Option<Holder<LootItemCondition>>, entity: EntityId| {
            simple_criterion_trigger::matches_entity(level, player, predicate.as_ref(), entity)
        };
        matches(level, &self.parent, parent) && matches(level, &self.partner, partner) || matches(level, &self.parent, partner) && matches(level, &self.partner, parent)
    }
}

impl TriggerInstance {
    pub fn validate(&self, validator: &mut ValidationContextSource) {
        simple_criterion_trigger::validate(validator, self.player.as_ref());
        validatable::validate_optional_holder(validator.entity_context(), "parent", self.parent.as_ref());
        validatable::validate_optional_holder(validator.entity_context(), "partner", self.partner.as_ref());
        validatable::validate_optional_holder(validator.entity_context(), "child", self.child.as_ref());
    }
}
