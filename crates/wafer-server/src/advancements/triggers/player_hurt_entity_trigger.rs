use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::damage_predicate::DamagePredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::damagesource::DamageSource;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validation_context_source::ValidationContextSource;
use crate::world::level::storage::loot::validatable;

#[derive(Clone, Copy)]
pub struct PlayerHurtEntityTrigger(pub CriterionTrigger);

impl PlayerHurtEntityTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, victim: EntityId, source: &DamageSource, original_damage: f32, actual_damage: f32, blocked: bool) {
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::PlayerHurtEntity(t) = instance else { return false };
            t.matches(level, player, victim, source, original_damage, actual_damage, blocked)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    damage: Option<DamagePredicate>,
    entity: Option<Holder<LootItemCondition>>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let damage = fields.optional("damage", DamagePredicate::decode);
            let entity = fields.condition("entity");
            Some(TriggerInstance { player, damage, entity })
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, victim: EntityId, source: &DamageSource, original_damage: f32, actual_damage: f32, blocked: bool) -> bool {
        if self.damage.as_ref().is_some_and(|damage| !damage.matches(level, player, source, original_damage, actual_damage, blocked)) {
            return false;
        }
        simple_criterion_trigger::matches_entity(level, player, self.entity.as_ref(), victim)
    }
}

impl TriggerInstance {
    pub fn validate(&self, validator: &mut ValidationContextSource) {
        simple_criterion_trigger::validate(validator, self.player.as_ref());
        validatable::validate_optional_holder(validator.entity_context(), "entity", self.entity.as_ref());
    }
}
