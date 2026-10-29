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

#[derive(Clone, Copy)]
pub struct ItemDurabilityTrigger(pub CriterionTrigger);

impl ItemDurabilityTrigger {
    pub fn trigger(self, level: &mut ServerLevel, player: EntityId, item_stack: &ItemStack, new_durability: i32) {
        simple_criterion_trigger::trigger(level, player, self.0, |_, instance| {
            let CriterionTriggerInstance::ItemDurability(t) = instance else { return false };
            t.matches(item_stack, new_durability)
        });
    }
}

pub struct TriggerInstance {
    pub player: Option<Holder<LootItemCondition>>,
    item: Option<ItemPredicate>,
    durability: Ints,
    delta: Ints,
}

impl TriggerInstance {
    pub fn decode(tag: &Nbt) -> DecodeResult<TriggerInstance> {
        simple_criterion_trigger::decode(tag, |fields| {
            let player = fields.player();
            let item = fields.optional("item", ItemPredicate::decode);
            let durability = fields.with_default("durability", Ints::ANY, Ints::decode);
            let delta = fields.with_default("delta", Ints::ANY, Ints::decode);
            Some(TriggerInstance { player, item, durability, delta })
        })
    }

    pub fn matches(&self, item_stack: &ItemStack, new_durability: i32) -> bool {
        if self.item.as_ref().is_some_and(|item| !item.test(item_stack)) {
            return false;
        }
        if !self.durability.matches(item_stack.max_damage() - new_durability) {
            return false;
        }
        self.delta.matches(item_stack.damage_value() - new_durability)
    }
}
