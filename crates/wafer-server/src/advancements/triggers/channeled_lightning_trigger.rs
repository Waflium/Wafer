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
pub struct ChanneledLightningTrigger(pub CriterionTrigger);

impl ChanneledLightningTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, victims: &[EntityId]) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::ChanneledLightning(t) = instance else { return false };
            t.matches(level, player, victims)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    victims: Vec<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let victims = fields.conditions("victims");
            Some(TriggerInstance { player, victims })
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, victims: &[EntityId]) -> bool {
        self.victims.iter().all(|predicate| victims.iter().any(|&victim| simple_criterion_trigger::matches_entity(level, player, Some(predicate), victim)))
    }
}

impl TriggerInstance {
    pub fn validate(&self, validator: &mut ValidationContextSource) {
        simple_criterion_trigger::validate(validator, self.player.as_ref());
        validatable::validate_holder_list(validator.entity_context(), "victims", &self.victims);
    }
}
