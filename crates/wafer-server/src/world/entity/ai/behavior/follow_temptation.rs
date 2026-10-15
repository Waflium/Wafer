use super::behavior::{Behavior, BehaviorState};
use super::entity_tracker::EntityTracker;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub const TEMPTATION_COOLDOWN: i32 = 100;
pub const DEFAULT_CLOSE_ENOUGH_DIST: f64 = 2.5;

pub struct FollowTemptation {
    behavior: BehaviorState,
    speed_modifier: Box<dyn Fn(&ServerLevel, EntityId) -> f32>,
    close_enough_distance: Box<dyn Fn(&ServerLevel, EntityId) -> f64>,
    look_in_the_eyes: bool,
}

impl FollowTemptation {
    pub fn new(speed_modifier: impl Fn(&ServerLevel, EntityId) -> f32 + 'static) -> FollowTemptation {
        FollowTemptation::with_close_enough_distance(speed_modifier, |_, _| DEFAULT_CLOSE_ENOUGH_DIST)
    }

    pub fn with_close_enough_distance(speed_modifier: impl Fn(&ServerLevel, EntityId) -> f32 + 'static, close_enough_distance: impl Fn(&ServerLevel, EntityId) -> f64 + 'static) -> FollowTemptation {
        FollowTemptation::with_look_in_the_eyes(speed_modifier, close_enough_distance, false)
    }

    pub fn with_look_in_the_eyes(
        speed_modifier: impl Fn(&ServerLevel, EntityId) -> f32 + 'static,
        close_enough_distance: impl Fn(&ServerLevel, EntityId) -> f64 + 'static,
        look_in_the_eyes: bool,
    ) -> FollowTemptation {
        FollowTemptation {
            behavior: BehaviorState::new(vec![
                (memory_module_type::LOOK_TARGET.erase(), MemoryStatus::Registered),
                (memory_module_type::WALK_TARGET.erase(), MemoryStatus::Registered),
                (memory_module_type::TEMPTATION_COOLDOWN_TICKS.erase(), MemoryStatus::ValueAbsent),
                (memory_module_type::TEMPTING_PLAYER.erase(), MemoryStatus::ValuePresent),
                (memory_module_type::BREED_TARGET.erase(), MemoryStatus::ValueAbsent),
                (memory_module_type::IS_PANICKING.erase(), MemoryStatus::ValueAbsent),
            ]),
            speed_modifier: Box::new(speed_modifier),
            close_enough_distance: Box::new(close_enough_distance),
            look_in_the_eyes,
        }
    }

    fn get_speed_modifier(&self, level: &ServerLevel, body: EntityId) -> f32 {
        (self.speed_modifier)(level, body)
    }

    fn get_tempting_player(&self, level: &ServerLevel, body: EntityId) -> Option<EntityId> {
        level.get_brain(body).get_memory(memory_module_type::TEMPTING_PLAYER).copied()
    }
}

impl Behavior for FollowTemptation {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn timed_out(&self, _timestamp: i64) -> bool {
        false
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        let brain = level.get_brain(body);
        self.get_tempting_player(level, body).is_some_and(|player| level.entity_ref(player).is_some())
            && !brain.has_memory_value(memory_module_type::BREED_TARGET)
            && !brain.has_memory_value(memory_module_type::IS_PANICKING)
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let brain = level.get_brain_mut(body);
        brain.set_memory(memory_module_type::TEMPTATION_COOLDOWN_TICKS, TEMPTATION_COOLDOWN);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        brain.erase_memory(memory_module_type::LOOK_TARGET);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let player = self.get_tempting_player(level, body).expect("a tempting player");
        let look_target = PositionTracker::Entity(EntityTracker::new(level, player, true));
        level.get_brain_mut(body).set_memory(memory_module_type::LOOK_TARGET, look_target);
        let close_enough = (self.close_enough_distance)(level, body);
        if level.entity(body).distance_to_sqr(level.entity(player)) < close_enough * close_enough {
            level.get_brain_mut(body).erase_memory(memory_module_type::WALK_TARGET);
        } else {
            let tracker = EntityTracker::with_target_eye_height(level, player, self.look_in_the_eyes, self.look_in_the_eyes);
            let walk_target = WalkTarget::new(PositionTracker::Entity(tracker), self.get_speed_modifier(level, body), 2);
            level.get_brain_mut(body).set_memory(memory_module_type::WALK_TARGET, walk_target);
        }
    }
}
