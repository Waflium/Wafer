use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::{self, DecodeResult};
use crate::world::entity::EntityId;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct ChangeDimensionTrigger(pub CriterionTrigger);

impl ChangeDimensionTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, from: &str, to: &str) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::ChangeDimension(t) = instance else { return false };
            t.matches(from, to)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    from: Option<String>,
    to: Option<String>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let from = fields.optional("from", codec::identifier);
            let to = fields.optional("to", codec::identifier);
            Some(TriggerInstance { player, from, to })
        })
    }

    pub fn matches(&self, from: &str, to: &str) -> bool {
        if self.from.as_deref().is_some_and(|expected| expected != from) {
            return false;
        }
        self.to.as_deref().is_none_or(|expected| expected == to)
    }
}
