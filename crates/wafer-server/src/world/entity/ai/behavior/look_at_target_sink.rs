use super::behavior::{Behavior, BehaviorState};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

pub struct LookAtTargetSink {
    behavior: BehaviorState,
}

impl LookAtTargetSink {
    pub fn new(min_duration: i32, max_duration: i32) -> LookAtTargetSink {
        LookAtTargetSink { behavior: BehaviorState::with_durations(vec![(memory_module_type::LOOK_TARGET.erase(), MemoryStatus::ValuePresent)], min_duration, max_duration) }
    }
}

impl Behavior for LookAtTargetSink {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        match level.get_brain(body).get_memory(memory_module_type::LOOK_TARGET).cloned() {
            Some(pos) => pos.is_visible_by(level, body),
            None => false,
        }
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        level.get_brain_mut(body).erase_memory(memory_module_type::LOOK_TARGET);
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if let Some(target) = level.get_brain(body).get_memory(memory_module_type::LOOK_TARGET).cloned() {
            let position = target.current_position(level);
            level.set_look_at(body, position.x, position.y, position.z);
        }
    }
}
