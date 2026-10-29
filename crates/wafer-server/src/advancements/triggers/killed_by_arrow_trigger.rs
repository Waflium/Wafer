use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::advancements::predicates::min_max_bounds::Ints;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::validation_context_source::ValidationContextSource;
use crate::world::level::storage::loot::validatable;

#[derive(Clone, Copy)]
pub struct KilledByArrowTrigger(pub CriterionTrigger);

impl KilledByArrowTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, victims: &[EntityId], fired_by_weapon: Option<&ItemStack>) {
        let mut entity_types: Vec<i32> = Vec::new();
        for &victim in victims {
            let entity_type = level.entity(victim).entity_type;
            if !entity_types.contains(&entity_type) {
                entity_types.push(entity_type);
            }
        }
        let unique_entity_types = entity_types.len() as i32;
        simple_criterion_trigger::trigger(level, player, self.0, |level, instance| {
            let CriterionTriggerInstance::KilledByArrow(t) = instance else { return false };
            t.matches(level, player, victims, unique_entity_types, fired_by_weapon)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    victims: Vec<Holder<LootItemCondition>>,
    unique_entity_types: Ints,
    fired_from_weapon: Option<ItemPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let victims = fields.conditions("victims");
            let unique_entity_types = fields.with_default("unique_entity_types", Ints::ANY, Ints::decode);
            let fired_from_weapon = fields.optional("fired_from_weapon", ItemPredicate::decode);
            Some(TriggerInstance { player, victims, unique_entity_types, fired_from_weapon })
        })
    }

    pub fn matches(&self, level: &mut ServerLevel, player: EntityId, victims: &[EntityId], unique_entity_types: i32, fired_from_weapon: Option<&ItemStack>) -> bool {
        if let Some(predicate) = &self.fired_from_weapon
            && !fired_from_weapon.is_some_and(|weapon| predicate.test(weapon))
        {
            return false;
        }
        if !self.victims.is_empty() {
            let mut victims_copy: Vec<EntityId> = victims.to_vec();
            for predicate in &self.victims {
                match victims_copy.iter().position(|&entity| simple_criterion_trigger::matches_entity(level, player, Some(predicate), entity)) {
                    Some(found) => {
                        victims_copy.remove(found);
                    }
                    None => return false,
                }
            }
        }
        self.unique_entity_types.matches(unique_entity_types)
    }
}

impl TriggerInstance {
    pub fn validate(&self, validator: &mut ValidationContextSource) {
        simple_criterion_trigger::validate(validator, self.player.as_ref());
        validatable::validate_holder_list(validator.entity_context(), "victims", &self.victims);
    }
}
