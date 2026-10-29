use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::component::predicates::potions_predicate::PotionsPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct BrewedPotionTrigger(pub CriterionTrigger);

impl BrewedPotionTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, potion: &Nbt) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::BrewedPotion(t) = instance else { return false };
            t.matches(potion)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    potion: Option<PotionsPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let potion = fields.optional("potion", PotionsPredicate::decode);
            Some(TriggerInstance { player, potion })
        })
    }

    pub fn matches(&self, potion: &Nbt) -> bool {
        self.potion.as_ref().is_none_or(|predicate| predicate.matches(potion))
    }
}
