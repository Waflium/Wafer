use wafer_core::random::RandomSource;
use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use super::behavior::{Behavior, BehaviorState};
use super::behavior_utils;
use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::projectile::projectile_util;
use crate::world::item::component as dc;
use crate::world::item::{ItemStack, crossbow_item};

const TIMEOUT: i32 = 1200;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CrossbowState {
    Uncharged,
    Charging,
    Charged,
    ReadyToAttack,
}

pub struct CrossbowAttack {
    state: BehaviorState,
    attack_delay: i32,
    crossbow_state: CrossbowState,
}

impl Default for CrossbowAttack {
    fn default() -> CrossbowAttack {
        let entry_condition = vec![(memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered), (memory_module_type::ATTACK_TARGET.erase(), MemoryStatus::ValuePresent)];
        CrossbowAttack { state: BehaviorState::with_time_out(entry_condition, TIMEOUT), attack_delay: 0, crossbow_state: CrossbowState::Uncharged }
    }
}

fn get_attack_target(level: &ServerLevel, body: EntityId) -> EntityId {
    *level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).expect("an attack target")
}

impl CrossbowAttack {
    fn crossbow_attack(&mut self, level: &mut ServerLevel, body: EntityId, target: EntityId) {
        match self.crossbow_state {
            CrossbowState::Uncharged => {
                let hand = projectile_util::get_weapon_holding_hand(level, body, items::CROSSBOW);
                level.start_using_item(body, hand);
                self.crossbow_state = CrossbowState::Charging;
                level.set_charging_crossbow(body, true);
            }
            CrossbowState::Charging => {
                if !level.is_using_item(body) {
                    self.crossbow_state = CrossbowState::Uncharged;
                }
                let pull_time = level.get_ticks_using_item(body);
                let use_item = level.living(body).use_item.clone().unwrap_or_else(ItemStack::empty);
                if pull_time >= crossbow_item::get_charge_duration(level, &use_item, body) {
                    level.release_using_item(body);
                    self.crossbow_state = CrossbowState::Charged;
                    self.attack_delay = 20 + level.entity_mut(body).random.next_int_bounded(20);
                    level.set_charging_crossbow(body, false);
                }
            }
            CrossbowState::Charged => {
                self.attack_delay -= 1;
                if self.attack_delay == 0 {
                    self.crossbow_state = CrossbowState::ReadyToAttack;
                }
            }
            CrossbowState::ReadyToAttack => {
                level.perform_ranged_attack(body, target, 1.0);
                self.crossbow_state = CrossbowState::Uncharged;
            }
        }
    }
}

impl Behavior for CrossbowAttack {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        let attack_target = get_attack_target(level, body);
        level.is_holding(body, items::CROSSBOW) && behavior_utils::entity_is_visible(level, body, attack_target) && behavior_utils::is_within_attack_range(level, body, attack_target, 0)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        level.get_brain(body).has_memory_value(memory_module_type::ATTACK_TARGET) && self.check_extra_start_conditions(level, body)
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let target = get_attack_target(level, body);
        let tracker = EntityTracker::new(level, target, true);
        level.get_brain_mut(body).set_memory(memory_module_type::LOOK_TARGET, PositionTracker::Entity(tracker));
        self.crossbow_attack(level, body, target);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if level.is_using_item(body) {
            level.stop_using_item(body);
        }
        if level.is_holding(body, items::CROSSBOW) {
            level.set_charging_crossbow(body, false);
            if let Some(use_item) = level.living_mut(body).use_item.as_mut() {
                use_item.set(dc::CHARGED_PROJECTILES, Some(Nbt::List(Vec::new())));
            }
        }
    }
}
