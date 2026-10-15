use super::behavior::{Behavior, BehaviorState};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::schedule::Activity;

pub struct VillagerPanicTrigger {
    behavior: BehaviorState,
}

impl Default for VillagerPanicTrigger {
    fn default() -> VillagerPanicTrigger {
        VillagerPanicTrigger { behavior: BehaviorState::new(Vec::new()) }
    }
}

pub fn has_hostile(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::NEAREST_HOSTILE)
}

pub fn is_hurt(level: &ServerLevel, body: EntityId) -> bool {
    level.get_brain(body).has_memory_value(memory_module_type::HURT_BY)
}

impl Behavior for VillagerPanicTrigger {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        is_hurt(level, body) || has_hostile(level, body)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if is_hurt(level, body) || has_hostile(level, body) {
            let brain = level.get_brain_mut(body);
            if !brain.is_active(Activity::PANIC) {
                brain.erase_memory(memory_module_type::PATH);
                brain.erase_memory(memory_module_type::WALK_TARGET);
                brain.erase_memory(memory_module_type::LOOK_TARGET);
                brain.erase_memory(memory_module_type::BREED_TARGET);
                brain.erase_memory(memory_module_type::INTERACTION_TARGET);
            }
            brain.set_active_activity_if_possible(Activity::PANIC);
        }
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        if timestamp % 100 == 0 {
            level.spawn_golem_if_needed(body, timestamp, 3);
        }
    }
}
