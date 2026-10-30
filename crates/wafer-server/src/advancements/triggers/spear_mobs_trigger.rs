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
pub struct SpearMobsTrigger(pub CriterionTrigger);

impl SpearMobsTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, number: i32) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::SpearMobs(t) = instance else { return false };
            t.matches(number)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    count: Option<i32>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let count = fields.optional("count", codec::positive_int);
            Some(TriggerInstance { player, count })
        })
    }

    pub fn matches(&self, required_count: i32) -> bool {
        self.count.is_none_or(|count| required_count >= count)
    }
}
