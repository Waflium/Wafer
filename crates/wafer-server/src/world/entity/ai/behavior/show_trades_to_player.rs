use wafer_data::EquipmentSlot;

use super::behavior::{Behavior, BehaviorState};
use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::mob::DEFAULT_EQUIPMENT_DROP_CHANCE;
use crate::world::entity::{EntityId, entity_types};
use crate::world::item::ItemStack;

const MAX_LOOK_TIME: i32 = 900;
const STARTING_LOOK_TIME: i32 = 40;

pub struct ShowTradesToPlayer {
    behavior: BehaviorState,
    player_item_stack: Option<(usize, ItemStack)>,
    display_items: Vec<ItemStack>,
    cycle_counter: i32,
    display_index: usize,
    look_time: i32,
}

impl ShowTradesToPlayer {
    pub fn new(min_duration: i32, max_duration: i32) -> ShowTradesToPlayer {
        ShowTradesToPlayer {
            behavior: BehaviorState::with_durations(vec![(memory_module_type::INTERACTION_TARGET.erase(), MemoryStatus::ValuePresent)], min_duration, max_duration),
            player_item_stack: None,
            display_items: Vec::new(),
            cycle_counter: 0,
            display_index: 0,
            look_time: 0,
        }
    }

    fn find_items_to_display(&mut self, level: &mut ServerLevel, player: EntityId, villager: EntityId) {
        let mut changed = false;
        let current_player_item_stack = level.item_by_slot(player, EquipmentSlot::Mainhand).unwrap_or_else(ItemStack::empty);
        if self.player_item_stack.as_ref().is_none_or(|(slot, stack)| !ItemStack::is_same_item(&held_stack(level, player, *slot, stack), &current_player_item_stack)) {
            self.player_item_stack = Some((level.player(player).inventory.selected_slot(), current_player_item_stack));
            changed = true;
            self.display_items.clear();
        }
        if changed && !self.player_item_stack.as_ref().expect("a player item").1.is_empty() {
            self.update_display_items(level, villager);
            if !self.display_items.is_empty() {
                self.look_time = MAX_LOOK_TIME;
                self.display_first_item(level, villager);
            }
        }
    }

    fn display_first_item(&self, level: &mut ServerLevel, villager: EntityId) {
        display_as_held_item(level, villager, self.display_items[0].clone());
    }

    fn update_display_items(&mut self, level: &mut ServerLevel, villager: EntityId) {
        let player_item_stack = self.player_item_stack.clone().expect("a player item").1;
        for offer in &level.get_offers(villager).0 {
            if !offer.is_out_of_stock() && (ItemStack::is_same_item(&player_item_stack, &offer.get_cost_a()) || ItemStack::is_same_item(&player_item_stack, &offer.get_cost_b())) {
                self.display_items.push(offer.assemble());
            }
        }
    }

    fn look_at_target(level: &mut ServerLevel, body: EntityId) -> EntityId {
        let target = *level.get_brain(body).get_memory(memory_module_type::INTERACTION_TARGET).expect("an interaction target");
        let tracker = EntityTracker::new(level, target, true);
        level.get_brain_mut(body).set_memory(memory_module_type::LOOK_TARGET, PositionTracker::Entity(tracker));
        target
    }

    fn display_cycling_items(&mut self, level: &mut ServerLevel, villager: EntityId) {
        if self.display_items.len() >= 2 {
            self.cycle_counter += 1;
            if self.cycle_counter >= 40 {
                self.display_index += 1;
                self.cycle_counter = 0;
                if self.display_index > self.display_items.len() - 1 {
                    self.display_index = 0;
                }
                display_as_held_item(level, villager, self.display_items[self.display_index].clone());
            }
        }
    }
}

fn held_stack(level: &ServerLevel, player: EntityId, slot: usize, stack: &ItemStack) -> ItemStack {
    if ItemStack::is_same_item(&level.player(player).inventory.get_item(slot), stack) { stack.clone() } else { ItemStack::empty() }
}

fn clear_held_item(level: &mut ServerLevel, body: EntityId) {
    level.living_set_item_slot(body, EquipmentSlot::Mainhand, ItemStack::empty());
    level.mob_mut(body).expect("a villager").set_drop_chance(EquipmentSlot::Mainhand, DEFAULT_EQUIPMENT_DROP_CHANCE);
}

fn display_as_held_item(level: &mut ServerLevel, body: EntityId, item_stack: ItemStack) {
    level.living_set_item_slot(body, EquipmentSlot::Mainhand, item_stack);
    level.mob_mut(body).expect("a villager").set_drop_chance(EquipmentSlot::Mainhand, 0.0);
}

impl Behavior for ShowTradesToPlayer {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let Some(&target) = level.get_brain(body).get_memory(memory_module_type::INTERACTION_TARGET) else { return false };
        level.entity_ref(target).is_some_and(|e| e.entity_type == entity_types::PLAYER.id)
            && level.is_alive(body)
            && level.is_alive(target)
            && !level.is_baby(body)
            && level.entity(body).position().distance_to_sqr(level.entity(target).position()) <= 17.0
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        self.check_extra_start_conditions(level, body) && self.look_time > 0 && level.get_brain(body).get_memory(memory_module_type::INTERACTION_TARGET).is_some()
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        ShowTradesToPlayer::look_at_target(level, body);
        self.cycle_counter = 0;
        self.display_index = 0;
        self.look_time = STARTING_LOOK_TIME;
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let target = ShowTradesToPlayer::look_at_target(level, body);
        self.find_items_to_display(level, target, body);
        if !self.display_items.is_empty() {
            self.display_cycling_items(level, body);
        } else {
            clear_held_item(level, body);
            self.look_time = self.look_time.min(STARTING_LOOK_TIME);
        }
        self.look_time -= 1;
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_brain_mut(body).erase_memory(memory_module_type::INTERACTION_TARGET);
        clear_held_item(level, body);
        self.player_item_stack = None;
    }
}
