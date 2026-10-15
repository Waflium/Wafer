use super::behavior::Status;
use super::behavior_control::BehaviorControl;
use super::declarative::memory_condition::MemoryCondition;
use super::declarative::trigger::Trigger;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{Erased, MemoryModuleType};

pub struct OneShot {
    status: Status,
    conditions: Vec<MemoryCondition>,
    required_memories: Vec<MemoryModuleType<Erased>>,
    trigger: Box<dyn Trigger>,
}

impl OneShot {
    pub fn new(conditions: Vec<MemoryCondition>, trigger: Box<dyn Trigger>) -> OneShot {
        let required_memories = conditions.iter().map(|condition| condition.memory()).collect();
        OneShot::with_required_memories(conditions, required_memories, trigger)
    }

    pub fn with_required_memories(conditions: Vec<MemoryCondition>, required_memories: Vec<MemoryModuleType<Erased>>, trigger: Box<dyn Trigger>) -> OneShot {
        OneShot { status: Status::Stopped, conditions, required_memories, trigger }
    }
}

impl Trigger for OneShot {
    fn trigger(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        let brain = level.get_brain(body);
        if !self.conditions.iter().all(|condition| condition.create_accessor(brain)) {
            return false;
        }
        self.trigger.trigger(level, body, timestamp)
    }
}

impl BehaviorControl for OneShot {
    fn get_status(&self) -> Status {
        self.status
    }

    fn get_required_memories(&self) -> Vec<MemoryModuleType<Erased>> {
        self.required_memories.clone()
    }

    fn try_start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        if self.trigger(level, body, timestamp) {
            self.status = Status::Running;
            true
        } else {
            false
        }
    }

    fn tick_or_stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.do_stop(level, body, timestamp);
    }

    fn do_stop(&mut self, _level: &mut ServerLevel, _body: EntityId, _timestamp: i64) {
        self.status = Status::Stopped;
    }

    fn debug_string(&self) -> String {
        "OneShot".to_string()
    }
}
