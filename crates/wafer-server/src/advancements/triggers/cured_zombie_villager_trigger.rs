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
pub struct CuredZombieVillagerTrigger(pub CriterionTrigger);

impl CuredZombieVillagerTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, zombie: EntityId, villager: EntityId) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::CuredZombieVillager(t) = instance else { return false };
            t.matches(level, player, zombie, villager)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    zombie: Option<Holder<LootItemCondition>>,
    villager: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let zombie = fields.condition("zombie");
            let villager = fields.condition("villager");
            Some(TriggerInstance { player, zombie, villager })
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, zombie: EntityId, villager: EntityId) -> bool {
        simple_criterion_trigger::matches_entity(level, player, self.zombie.as_ref(), zombie) && simple_criterion_trigger::matches_entity(level, player, self.villager.as_ref(), villager)
    }
}

impl TriggerInstance {
    pub fn validate(&self, validator: &mut ValidationContextSource) {
        simple_criterion_trigger::validate(validator, self.player.as_ref());
        validatable::validate_optional_holder(validator.entity_context(), "zombie", self.zombie.as_ref());
        validatable::validate_optional_holder(validator.entity_context(), "villager", self.villager.as_ref());
    }
}
