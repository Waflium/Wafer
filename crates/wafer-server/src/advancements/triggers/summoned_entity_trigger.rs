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
pub struct SummonedEntityTrigger(pub CriterionTrigger);

impl SummonedEntityTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, entity: EntityId) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::SummonedEntity(t) = instance else { return false };
            simple_criterion_trigger::matches_entity(level, player, t.entity.as_ref(), entity)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    entity: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let entity = fields.condition("entity");
            Some(TriggerInstance { player, entity })
        })
    }
}

impl TriggerInstance {
    pub fn validate(&self, validator: &mut ValidationContextSource) {
        simple_criterion_trigger::validate(validator, self.player.as_ref());
        validatable::validate_optional_holder(validator.entity_context(), "entity", self.entity.as_ref());
    }
}
