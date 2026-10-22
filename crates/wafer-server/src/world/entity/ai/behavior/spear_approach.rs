use super::behavior::{Behavior, BehaviorState};
use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use super::spear_attack::{SpearStatus, able_to_attack, charge_speed_modifier_of_mount, get_target};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

pub struct SpearApproach {
    state: BehaviorState,
    speed_modifier_when_repositioning: f64,
    approach_distance_sq: f32,
}

impl SpearApproach {
    pub fn new(speed_modifier_when_repositioning: f64, approach_distance: f32) -> SpearApproach {
        SpearApproach {
            state: BehaviorState::new(vec![(memory_module_type::SPEAR_STATUS.erase(), MemoryStatus::ValueAbsent)]),
            speed_modifier_when_repositioning,
            approach_distance_sq: approach_distance * approach_distance,
        }
    }

    fn far_enough(&self, level: &ServerLevel, body: EntityId) -> bool {
        let Some(target) = get_target(level, body) else { return false };
        let t = level.entity(target);
        let target_dist_sqr = level.entity(body).distance_to_sqr_xyz(t.x(), t.y(), t.z());
        target_dist_sqr > self.approach_distance_sq as f64
    }
}

impl Behavior for SpearApproach {
    fn behavior(&self) -> &BehaviorState {
        &self.state
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.state
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        able_to_attack(level, body) && !level.is_using_item(body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.mob_mut(body).expect("a mob").set_aggressive(true);
        level.get_brain_mut(body).set_memory(memory_module_type::SPEAR_STATUS, SpearStatus::Approach);
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        able_to_attack(level, body) && self.far_enough(level, body)
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let Some(target) = get_target(level, body) else { return };
        let speed_modifier = charge_speed_modifier_of_mount(level, body);
        let tracker = EntityTracker::new(level, target, true);
        level.get_brain_mut(body).set_memory(memory_module_type::LOOK_TARGET, PositionTracker::Entity(tracker));
        let speed = speed_modifier as f64 * self.speed_modifier_when_repositioning;
        level.get_navigation(body, |navigation, level| navigation.move_to_entity(level, body, target, speed));
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.mob_mut(body).expect("a mob").ai.navigation.stop();
        level.get_brain_mut(body).set_memory(memory_module_type::SPEAR_STATUS, SpearStatus::Charging);
    }

    fn timed_out(&self, _timestamp: i64) -> bool {
        false
    }
}
