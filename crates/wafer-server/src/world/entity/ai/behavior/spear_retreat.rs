use super::behavior::{Behavior, BehaviorState};
use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use super::spear_attack::{SpearStatus, able_to_attack, charge_speed_modifier_of_mount, get_target};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::util::land_random_pos;

pub const MIN_COOLDOWN_DISTANCE: i32 = 9;
pub const MAX_COOLDOWN_DISTANCE: i32 = 11;
pub const MAX_FLEEING_TIME: i32 = 100;

pub struct SpearRetreat {
    state: BehaviorState,
    speed_modifier_when_repositioning: f64,
}

impl SpearRetreat {
    pub fn new(speed_modifier_when_repositioning: f64) -> SpearRetreat {
        SpearRetreat { state: BehaviorState::with_time_out(vec![(memory_module_type::SPEAR_STATUS.erase(), MemoryStatus::ValuePresent)], MAX_FLEEING_TIME), speed_modifier_when_repositioning }
    }
}

impl Behavior for SpearRetreat {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        if !able_to_attack(level, body) || level.is_using_item(body) {
            return false;
        }
        if level.get_brain(body).get_memory(memory_module_type::SPEAR_STATUS).copied().unwrap_or(SpearStatus::Approach) != SpearStatus::Retreat {
            return false;
        }
        let Some(target) = get_target(level, body) else { return false };
        let t = level.entity(target);
        let (target_x, target_y, target_z, target_position) = (t.x(), t.y(), t.z(), t.position());
        let target_dist_sqr = level.entity(body).distance_to_sqr_xyz(target_x, target_y, target_z);
        let mount_distance = if level.is_passenger(body) { 2 } else { 0 };
        let distance = target_dist_sqr.sqrt();
        let away_pos = land_random_pos::get_pos_away_between(
            level,
            body,
            0.0f64.max((MIN_COOLDOWN_DISTANCE + mount_distance) as f64 - distance),
            1.0f64.max((MAX_COOLDOWN_DISTANCE + mount_distance) as f64 - distance),
            7,
            target_position,
        );
        let Some(away_pos) = away_pos else { return false };
        level.get_brain_mut(body).set_memory(memory_module_type::SPEAR_FLEEING_POSITION, away_pos);
        true
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.mob_mut(body).expect("a mob").set_aggressive(true);
        level.get_brain_mut(body).set_memory(memory_module_type::SPEAR_FLEEING_TIME, 0);
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        let brain = level.get_brain(body);
        brain.get_memory(memory_module_type::SPEAR_FLEEING_TIME).copied().unwrap_or(MAX_FLEEING_TIME) < MAX_FLEEING_TIME
            && brain.get_memory(memory_module_type::SPEAR_FLEEING_POSITION).is_some()
            && !level.mob(body).expect("a mob").ai.navigation.is_done()
            && able_to_attack(level, body)
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let Some(target) = get_target(level, body) else { return };
        let speed_modifier = charge_speed_modifier_of_mount(level, body);
        let tracker = EntityTracker::new(level, target, true);
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::LOOK_TARGET, PositionTracker::Entity(tracker));
        let fleeing_time = brain.get_memory(memory_module_type::SPEAR_FLEEING_TIME).copied().unwrap_or(0) + 1;
        brain.set_memory(memory_module_type::SPEAR_FLEEING_TIME, fleeing_time);
        if let Some(flee_pos) = brain.get_memory(memory_module_type::SPEAR_FLEEING_POSITION).copied() {
            let speed = speed_modifier as f64 * self.speed_modifier_when_repositioning;
            level.get_navigation(body, |navigation, level| navigation.move_to(level, body, flee_pos.x, flee_pos.y, flee_pos.z, speed));
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let mob = level.mob_mut(body).expect("a mob");
        mob.ai.navigation.stop();
        mob.set_aggressive(false);
        level.stop_using_item(body);
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::SPEAR_FLEEING_TIME);
        brain.erase_memory(memory_module_type::SPEAR_FLEEING_POSITION);
        brain.erase_memory(memory_module_type::SPEAR_STATUS);
    }
}
