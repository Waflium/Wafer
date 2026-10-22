use wafer_data::EquipmentSlot;

use super::behavior::{Behavior, BehaviorState};
use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::util::land_random_pos;
use crate::world::item::component as dc;
use crate::world::item::component::kinetic_weapon::KineticWeapon;

pub const MIN_REPOSITION_DISTANCE: i32 = 6;
pub const MAX_REPOSITION_DISTANCE: i32 = 7;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpearStatus {
    Approach,
    Charging,
    Retreat,
}

pub fn get_target(level: &ServerLevel, body: EntityId) -> Option<EntityId> {
    level.get_brain(body).get_memory(memory_module_type::ATTACK_TARGET).copied()
}

pub fn able_to_attack(level: &ServerLevel, body: EntityId) -> bool {
    get_target(level, body).is_some() && level.item_by_slot_ref(body, EquipmentSlot::Mainhand).is_some_and(|item| item.has(dc::KINETIC_WEAPON))
}

pub fn charge_speed_modifier_of_mount(level: &ServerLevel, body: EntityId) -> f32 {
    let mount = level.root_vehicle(body);
    if level.mob(mount).is_some() { level.charge_speed_modifier(mount) } else { 1.0 }
}

pub struct SpearAttack {
    state: BehaviorState,
    speed_modifier_when_charging: f64,
    speed_modifier_when_repositioning: f64,
    target_in_range_radius_sq: f32,
}

impl SpearAttack {
    pub fn new(speed_modifier_when_charging: f64, speed_modifier_when_repositioning: f64, target_in_range_radius: f32) -> SpearAttack {
        SpearAttack {
            state: BehaviorState::new(vec![(memory_module_type::SPEAR_STATUS.erase(), MemoryStatus::ValuePresent)]),
            speed_modifier_when_charging,
            speed_modifier_when_repositioning,
            target_in_range_radius_sq: target_in_range_radius * target_in_range_radius,
        }
    }

    fn get_kinetic_weapon_use_duration(&self, level: &ServerLevel, body: EntityId) -> i32 {
        level.item_by_slot_ref(body, EquipmentSlot::Mainhand).and_then(KineticWeapon::of).map_or(0, |kinetic_weapon| kinetic_weapon.compute_damage_use_duration())
    }
}

impl Behavior for SpearAttack {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.get_brain(body).get_memory(memory_module_type::SPEAR_STATUS).copied().unwrap_or(SpearStatus::Approach) == SpearStatus::Charging
            && able_to_attack(level, body)
            && !level.is_using_item(body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.mob_mut(body).expect("a mob").set_aggressive(true);
        let duration = self.get_kinetic_weapon_use_duration(level, body);
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::SPEAR_ENGAGE_TIME, duration);
        brain.erase_memory(memory_module_type::SPEAR_CHARGE_POSITION);
        level.start_using_item(body, InteractionHand::MainHand);
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        level.get_brain(body).get_memory(memory_module_type::SPEAR_ENGAGE_TIME).copied().unwrap_or(0) > 0 && able_to_attack(level, body)
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let Some(target) = get_target(level, body) else { return };
        let t = level.entity(target);
        let (target_x, target_y, target_z, target_position) = (t.x(), t.y(), t.z(), t.position());
        let target_dist_sqr = level.entity(body).distance_to_sqr_xyz(target_x, target_y, target_z);
        let speed_modifier = charge_speed_modifier_of_mount(level, body);
        let mount_distance = if level.is_passenger(body) { 2 } else { 0 };
        let tracker = EntityTracker::new(level, target, true);
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::LOOK_TARGET, PositionTracker::Entity(tracker));
        let engage_time = brain.get_memory(memory_module_type::SPEAR_ENGAGE_TIME).copied().unwrap_or(0) - 1;
        brain.set_memory(memory_module_type::SPEAR_ENGAGE_TIME, engage_time);
        match brain.get_memory(memory_module_type::SPEAR_CHARGE_POSITION).copied() {
            Some(away_pos) => {
                let speed = speed_modifier as f64 * self.speed_modifier_when_repositioning;
                level.get_navigation(body, |navigation, level| navigation.move_to(level, body, away_pos.x, away_pos.y, away_pos.z, speed));
                if level.mob(body).expect("a mob").ai.navigation.is_done() {
                    level.get_brain_mut(body).erase_memory(memory_module_type::SPEAR_CHARGE_POSITION);
                }
            }
            None => {
                let speed = speed_modifier as f64 * self.speed_modifier_when_charging;
                level.get_navigation(body, |navigation, level| navigation.move_to_entity(level, body, target, speed));
                if target_dist_sqr < self.target_in_range_radius_sq as f64 || level.mob(body).expect("a mob").ai.navigation.is_done() {
                    let distance = target_dist_sqr.sqrt();
                    let new_away_pos = land_random_pos::get_pos_away_between(
                        level,
                        body,
                        (MIN_REPOSITION_DISTANCE + mount_distance) as f64 - distance,
                        (MAX_REPOSITION_DISTANCE + mount_distance) as f64 - distance,
                        7,
                        target_position,
                    );
                    level.get_brain_mut(body).set_memory_optional(memory_module_type::SPEAR_CHARGE_POSITION, new_away_pos);
                }
            }
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.mob_mut(body).expect("a mob").ai.navigation.stop();
        level.stop_using_item(body);
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::SPEAR_CHARGE_POSITION);
        brain.erase_memory(memory_module_type::SPEAR_ENGAGE_TIME);
        brain.set_memory(memory_module_type::SPEAR_STATUS, SpearStatus::Retreat);
    }

    fn timed_out(&self, _timestamp: i64) -> bool {
        false
    }
}
