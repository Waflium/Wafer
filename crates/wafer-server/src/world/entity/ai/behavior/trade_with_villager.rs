use wafer_data::{Item, items};

use super::behavior::{Behavior, BehaviorState};
use super::behavior_utils;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::npc::villager::villager::{has_excess_food, wants_more_food};
use crate::world::entity::npc::villager::villager_profession::ProfessionId;
use crate::world::entity::{EntityId, entity_types};
use crate::world::item::ItemStack;
use crate::world::item::component as dc;

pub struct TradeWithVillager {
    behavior: BehaviorState,
    trades: Vec<Item>,
}

impl Default for TradeWithVillager {
    fn default() -> TradeWithVillager {
        TradeWithVillager {
            behavior: BehaviorState::new(vec![
                (memory_module_type::INTERACTION_TARGET.erase(), MemoryStatus::ValuePresent),
                (memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase(), MemoryStatus::ValuePresent),
            ]),
            trades: Vec::new(),
        }
    }
}

fn figure_out_what_i_am_willing_to_trade(level: &ServerLevel, my_body: EntityId, target: EntityId) -> Vec<Item> {
    let target_items = level.get_villager_data(target).profession.value().requested_items;
    let self_items = level.get_villager_data(my_body).profession.value().requested_items;
    target_items.iter().copied().filter(|entry| !self_items.contains(entry)).collect()
}

fn throw_half_stack(level: &mut ServerLevel, villager: EntityId, predicate: impl Fn(&ItemStack) -> bool, target: EntityId) {
    let inventory = &mut level.abstract_villager_mut(villager).inventory;
    let mut to_throw = ItemStack::empty();
    for i in 0..inventory.get_container_size() {
        let item_stack = &mut inventory.items[i];
        if !item_stack.is_empty() && predicate(item_stack) {
            let count = if item_stack.count > item_stack.max_stack_size() / 2 {
                Some(item_stack.count / 2)
            } else if item_stack.count > 24 {
                Some(item_stack.count - 24)
            } else {
                None
            };
            if let Some(count) = count {
                to_throw = item_stack.split(count);
                break;
            }
        }
    }
    if !to_throw.is_empty() {
        let target_pos = level.entity(target).position();
        behavior_utils::throw_item(level, villager, to_throw, target_pos);
    }
}

impl Behavior for TradeWithVillager {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        behavior_utils::target_is_valid(level, body, memory_module_type::INTERACTION_TARGET, &entity_types::VILLAGER)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        self.check_extra_start_conditions(level, body)
    }

    fn start(&mut self, level: &mut ServerLevel, my_body: EntityId, _timestamp: i64) {
        let target = *level.get_brain(my_body).get_memory(memory_module_type::INTERACTION_TARGET).expect("an interaction target");
        behavior_utils::lock_gaze_and_walk_to_each_other(level, my_body, target, 0.5, 2);
        self.trades = figure_out_what_i_am_willing_to_trade(level, my_body, target);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        let target = *level.get_brain(body).get_memory(memory_module_type::INTERACTION_TARGET).expect("an interaction target");
        if level.entity(body).position().distance_to_sqr(level.entity(target).position()) > 5.0 {
            return;
        }
        behavior_utils::lock_gaze_and_walk_to_each_other(level, body, target, 0.5, 2);
        level.gossip(body, target, timestamp);
        let is_farmer = level.get_villager_data(body).profession == ProfessionId::FARMER;
        if has_excess_food(level.mob(body).expect("a villager")) && (is_farmer || wants_more_food(level.mob(target).expect("a villager"))) {
            throw_half_stack(level, body, |item_stack| item_stack.has(dc::VILLAGER_FOOD), target);
        }
        if is_farmer && level.abstract_villager(body).inventory.count_item(items::WHEAT) > ItemStack::new(items::WHEAT, 1).max_stack_size() / 2 {
            throw_half_stack(level, body, |item_stack| item_stack.is(items::WHEAT), target);
        }
        if !self.trades.is_empty() && level.abstract_villager(body).inventory.has_any_matching(|item| self.trades.contains(&item.get_item())) {
            let trades = self.trades.clone();
            throw_half_stack(level, body, move |item_stack| trades.contains(&item_stack.get_item()), target);
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_brain_mut(body).erase_memory(memory_module_type::INTERACTION_TARGET);
    }
}
