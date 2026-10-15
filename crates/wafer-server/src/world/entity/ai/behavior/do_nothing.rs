use wafer_core::random::RandomSource;

use super::behavior::Status;
use super::behavior_control::BehaviorControl;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{Erased, MemoryModuleType};

pub struct DoNothing {
    min_duration: i32,
    max_duration: i32,
    status: Status,
    end_timestamp: i64,
}

impl DoNothing {
    pub fn new(min_duration: i32, max_duration: i32) -> DoNothing {
        DoNothing { min_duration, max_duration, status: Status::Stopped, end_timestamp: 0 }
    }
}

impl BehaviorControl for DoNothing {
    fn get_status(&self) -> Status {
        self.status
    }

    fn get_required_memories(&self) -> Vec<MemoryModuleType<Erased>> {
        Vec::new()
    }

    fn try_start(&mut self, level: &mut ServerLevel, _body: EntityId, timestamp: i64) -> bool {
        self.status = Status::Running;
        let duration = self.min_duration + level.random.next_int_bounded(self.max_duration + 1 - self.min_duration);
        self.end_timestamp = timestamp + duration as i64;
        true
    }

    fn tick_or_stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        if timestamp > self.end_timestamp {
            self.do_stop(level, body, timestamp);
        }
    }

    fn do_stop(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) {
        self.status = Status::Stopped;
    }

    fn debug_string(&self) -> String {
        "DoNothing".to_string()
    }
}
