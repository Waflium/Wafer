use wafer_protocol::nbt::Nbt;

use super::criterion_trigger::CriterionTrigger;
use super::simple_criterion_trigger;
use crate::advancements::criterion_trigger_instance::CriterionTriggerInstance;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::core::holder::Holder;
use crate::server::level::ServerLevel;
use crate::util::codec::DecodeResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;

#[derive(Clone, Copy)]
pub struct UsingItemTrigger(pub CriterionTrigger);

impl UsingItemTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, item: &ItemStack) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::UsingItem(t) = instance else { return false };
            t.matches(item)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    item: Option<ItemPredicate>,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let item = fields.optional("item", ItemPredicate::decode);
            Some(TriggerInstance { player, item })
        })
    }

    pub fn matches(&self, item: &ItemStack) -> bool {
        self.item.as_ref().is_none_or(|predicate| predicate.test(item))
    }
}
