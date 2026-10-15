use wafer_core::random::RandomSource;

use super::behavior_control::BehaviorControl;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{Erased, MemoryModuleType};
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

pub const DEFAULT_DURATION: i32 = 60;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Status {
    Stopped,
    Running,
}

pub struct BehaviorState {
    entry_condition: Vec<(MemoryModuleType<Erased>, MemoryStatus)>,
    status: Status,
    end_timestamp: i64,
    min_duration: i32,
    max_duration: i32,
}

impl BehaviorState {
    pub fn new(entry_condition: Vec<(MemoryModuleType<Erased>, MemoryStatus)>) -> BehaviorState {
        BehaviorState::with_time_out(entry_condition, DEFAULT_DURATION)
    }

    pub fn with_time_out(entry_condition: Vec<(MemoryModuleType<Erased>, MemoryStatus)>, time_out_duration: i32) -> BehaviorState {
        BehaviorState::with_durations(entry_condition, time_out_duration, time_out_duration)
    }

    pub fn with_durations(entry_condition: Vec<(MemoryModuleType<Erased>, MemoryStatus)>, min_duration: i32, max_duration: i32) -> BehaviorState {
        BehaviorState { entry_condition, status: Status::Stopped, end_timestamp: 0, min_duration, max_duration }
    }
}

pub trait Behavior {
    fn behavior(&self) -> &BehaviorState;

    fn behavior_mut(&mut self) -> &mut BehaviorState;

    fn start(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) {}

    fn tick(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) {}

    fn stop(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) {}

    fn can_still_use(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) -> bool {
        false
    }

    fn timed_out(&self, timestamp: i64) -> bool {
        timestamp > self.behavior().end_timestamp
    }

    fn check_extra_start_conditions(&mut self, _level: &mut ServerLevel, _body: EntityId) -> bool {
        true
    }

    fn debug_string(&self) -> String {
        std::any::type_name::<Self>().rsplit("::").next().unwrap_or_default().to_string()
    }

    fn has_required_memories(&self, level: &ServerLevel, body: EntityId) -> bool {
        let brain = level.get_brain(body);
        self.behavior().entry_condition.iter().all(|&(memory_type, required_status)| brain.check_memory(memory_type, required_status))
    }
}

impl<T: Behavior + 'static> BehaviorControl for T {
    fn get_status(&self) -> Status {
        self.behavior().status
    }

    fn get_required_memories(&self) -> Vec<MemoryModuleType<Erased>> {
        self.behavior().entry_condition.iter().map(|&(memory_type, _)| memory_type).collect()
    }

    fn try_start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        if self.has_required_memories(level, body) && self.check_extra_start_conditions(level, body) {
            let state = self.behavior_mut();
            state.status = Status::Running;
            let (min_duration, max_duration) = (state.min_duration, state.max_duration);
            let duration = min_duration + level.random.next_int_bounded(max_duration + 1 - min_duration);
            self.behavior_mut().end_timestamp = timestamp + duration as i64;
            self.start(level, body, timestamp);
            true
        } else {
            false
        }
    }

    fn tick_or_stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        if !self.timed_out(timestamp) && self.can_still_use(level, body, timestamp) {
            self.tick(level, body, timestamp);
        } else {
            self.do_stop(level, body, timestamp);
        }
    }

    fn do_stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.behavior_mut().status = Status::Stopped;
        self.stop(level, body, timestamp);
    }

    fn debug_string(&self) -> String {
        Behavior::debug_string(self)
    }
}
