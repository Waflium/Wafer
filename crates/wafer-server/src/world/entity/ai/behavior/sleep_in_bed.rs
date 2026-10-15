use wafer_data::block_tags;
use wafer_data::properties::OCCUPIED;

use super::behavior::{Behavior, BehaviorState};
use super::interact_with_door;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::memory_status::MemoryStatus;
use crate::world::entity::schedule::Activity;
use crate::world::level::BlockGetter;

pub const COOLDOWN_AFTER_BEING_WOKEN: i64 = 100;

pub struct SleepInBed {
    behavior: BehaviorState,
    next_ok_start_time: i64,
}

impl Default for SleepInBed {
    fn default() -> SleepInBed {
        SleepInBed {
            behavior: BehaviorState::new(vec![
                (memory_module_type::HOME.erase(), MemoryStatus::ValuePresent),
                (memory_module_type::LAST_WOKEN.erase(), MemoryStatus::Registered),
                (memory_module_type::LAST_SLEPT.erase(), MemoryStatus::Registered),
                (memory_module_type::WALK_TARGET.erase(), MemoryStatus::Registered),
                (memory_module_type::CANT_REACH_WALK_TARGET_SINCE.erase(), MemoryStatus::Registered),
            ]),
            next_ok_start_time: 0,
        }
    }
}

impl Behavior for SleepInBed {
    fn behavior(&self) -> &BehaviorState {
        &self.behavior
    }

    fn behavior_mut(&mut self) -> &mut BehaviorState {
        &mut self.behavior
    }

    fn check_extra_start_conditions(&mut self, level: &mut ServerLevel, body: EntityId) -> bool {
        if level.is_passenger(body) {
            return false;
        }
        let brain = level.get_brain(body);
        let target = brain.get_memory(memory_module_type::HOME).cloned().expect("a home");
        if level.dimension != target.dimension {
            return false;
        }
        if let Some(&last_woken) = brain.get_memory(memory_module_type::LAST_WOKEN) {
            let time_since_last_woken = level.game_time - last_woken;
            if time_since_last_woken > 0 && time_since_last_woken < COOLDOWN_AFTER_BEING_WOKEN {
                return false;
            }
        }
        let block_state = level.block_state(target.pos);
        target.pos.closer_to_center_than(level.entity(body).position(), 2.0) && block_state.block().is(&block_tags::VILLAGERS_CAN_SLEEP_ON_BED) && !block_state.get(OCCUPIED)
    }

    fn can_still_use(&mut self, level: &mut ServerLevel, body: EntityId, _timestamp: i64) -> bool {
        let brain = level.get_brain(body);
        let Some(memory) = brain.get_memory(memory_module_type::HOME) else { return false };
        let bed_pos = memory.pos;
        let e = level.entity(body);
        brain.is_active(Activity::REST) && e.y() > bed_pos.y as f64 + 0.4 && bed_pos.closer_to_center_than(e.position(), 1.14)
    }

    fn start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        if timestamp <= self.next_ok_start_time {
            return;
        }
        let brain = level.get_brain(body);
        if brain.has_memory_value(memory_module_type::DOORS_TO_CLOSE) {
            let mut doors = brain.get_memory(memory_module_type::DOORS_TO_CLOSE).cloned().expect("doors to close");
            let nearest_entities = if brain.has_memory_value(memory_module_type::NEAREST_LIVING_ENTITIES) { brain.get_memory(memory_module_type::NEAREST_LIVING_ENTITIES).cloned() } else { None };
            interact_with_door::close_doors_that_i_have_opened_or_passed_through(level, body, None, None, &mut doors, nearest_entities.as_deref());
            level.get_brain_mut(body).set_memory(memory_module_type::DOORS_TO_CLOSE, doors);
        }
        let home = level.get_brain(body).get_memory(memory_module_type::HOME).expect("a home").pos;
        if level.living_entity_start_sleeping(body, home) {
            level.get_brain_mut(body).set_memory(memory_module_type::LAST_SLEPT, timestamp);
        }
        let brain = level.get_brain_mut(body);
        brain.erase_memory(memory_module_type::WALK_TARGET);
        brain.erase_memory(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
    }

    fn timed_out(&self, _timestamp: i64) -> bool {
        false
    }

    fn stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        if level.is_sleeping(body) {
            level.stop_sleeping(body);
            self.next_ok_start_time = timestamp + 40;
        }
    }
}
