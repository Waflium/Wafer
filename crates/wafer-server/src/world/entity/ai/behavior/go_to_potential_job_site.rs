use super::behavior::{Behavior, BehaviorState};
use super::behavior_utils;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::schedule::Activity;

const TICKS_UNTIL_TIMEOUT: i32 = 1200;

pub struct GoToPotentialJobSite {
    behavior: BehaviorState,
    speed_modifier: f32,
}

impl GoToPotentialJobSite {
    pub fn new(speed_modifier: f32) -> GoToPotentialJobSite {
        GoToPotentialJobSite {
            behavior: BehaviorState::with_time_out(vec![(memory_module_type::POTENTIAL_JOB_SITE.erase(), MemoryStatus::ValuePresent)], TICKS_UNTIL_TIMEOUT),
            speed_modifier,
        }
    }
}

impl Behavior for GoToPotentialJobSite {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        level.get_brain(body).get_active_non_core_activity().is_none_or(|activity| activity == Activity::IDLE || activity == Activity::WORK || activity == Activity::PLAY)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        level.get_brain(body).has_memory_value(memory_module_type::POTENTIAL_JOB_SITE)
    }

    fn tick(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        let pos = level.get_brain(body).get_memory(memory_module_type::POTENTIAL_JOB_SITE).expect("a potential job site").pos;
        behavior_utils::set_walk_and_look_target_memories_to_pos(level, body, pos, self.speed_modifier, 1);
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) {
        if let Some(global_pos) = level.get_brain(body).get_memory(memory_module_type::POTENTIAL_JOB_SITE).cloned()
            && global_pos.dimension == level.dimension
        {
            let pos = global_pos.pos;
            if level.chunks.poi_manager.exists(pos, |_| true) {
                level.chunks.poi_manager.release(pos);
            }
        }
        level.get_brain_mut(body).erase_memory(memory_module_type::POTENTIAL_JOB_SITE);
    }
}
