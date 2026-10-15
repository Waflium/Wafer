use super::behavior::Status;
use super::behavior_control::BehaviorControl;
use super::shuffling_list::ShufflingList;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{Erased, MemoryModuleType};
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum OrderPolicy {
    Ordered,
    Shuffled,
}

impl OrderPolicy {
    pub fn apply<U>(self, list: &mut ShufflingList<U>) {
        if self == OrderPolicy::Shuffled {
            list.shuffle();
        }
    }
}

#[allow(dead_code)]
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RunningPolicy {
    RunOne,
    TryAll,
}

impl RunningPolicy {
    pub fn apply(self, behaviors: &mut ShufflingList<Box<dyn BehaviorControl>>, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        for behavior in behaviors.iter_mut() {
            if behavior.get_status() == Status::Stopped && behavior.try_start(level, body, timestamp) && self == RunningPolicy::RunOne {
                break;
            }
        }
    }
}

pub struct GateBehavior {
    entry_condition: Vec<(MemoryModuleType<Erased>, MemoryStatus)>,
    exit_erased_memories: Vec<MemoryModuleType<Erased>>,
    order_policy: OrderPolicy,
    running_policy: RunningPolicy,
    behaviors: ShufflingList<Box<dyn BehaviorControl>>,
    status: Status,
}

impl GateBehavior {
    pub fn new(
        entry_condition: Vec<(MemoryModuleType<Erased>, MemoryStatus)>,
        exit_erased_memories: Vec<MemoryModuleType<Erased>>,
        order_policy: OrderPolicy,
        running_policy: RunningPolicy,
        behaviors: Vec<(Box<dyn BehaviorControl>, i32)>,
    ) -> GateBehavior {
        let mut list = ShufflingList::default();
        for (behavior, weight) in behaviors {
            list.add(behavior, weight);
        }
        GateBehavior { entry_condition, exit_erased_memories, order_policy, running_policy, behaviors: list, status: Status::Stopped }
    }

    #[cfg(test)]
    pub fn set_seed(&mut self, seed: i64) {
        self.behaviors.set_seed(seed);
    }

    fn has_required_memories(&self, level: &ServerLevel, body: EntityId) -> bool {
        let brain = level.get_brain(body);
        self.entry_condition.iter().all(|&(memory_type, required_status)| brain.check_memory(memory_type, required_status))
    }
}

impl BehaviorControl for GateBehavior {
    fn get_status(&self) -> Status {
        self.status
    }

    fn get_required_memories(&self) -> Vec<MemoryModuleType<Erased>> {
        let mut memories: Vec<MemoryModuleType<Erased>> = self.entry_condition.iter().map(|&(memory_type, _)| memory_type).collect();
        for behavior in self.behaviors.iter() {
            for memory in behavior.get_required_memories() {
                if !memories.contains(&memory) {
                    memories.push(memory);
                }
            }
        }
        memories
    }

    fn try_start(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) -> bool {
        if self.has_required_memories(level, body) {
            self.status = Status::Running;
            self.order_policy.apply(&mut self.behaviors);
            self.running_policy.apply(&mut self.behaviors, level, body, timestamp);
            true
        } else {
            false
        }
    }

    fn tick_or_stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        for behavior in self.behaviors.iter_mut() {
            if behavior.get_status() == Status::Running {
                behavior.tick_or_stop(level, body, timestamp);
            }
        }
        if self.behaviors.iter().all(|behavior| behavior.get_status() != Status::Running) {
            self.do_stop(level, body, timestamp);
        }
    }

    fn do_stop(&mut self, level: &mut ServerLevel, body: EntityId, timestamp: i64) {
        self.status = Status::Stopped;
        for behavior in self.behaviors.iter_mut() {
            if behavior.get_status() == Status::Running {
                behavior.do_stop(level, body, timestamp);
            }
        }
        for &memory in &self.exit_erased_memories {
            level.get_brain_mut(body).erase_memory(memory);
        }
    }

    fn debug_string(&self) -> String {
        let running: Vec<String> = self.behaviors.iter().filter(|behavior| behavior.get_status() == Status::Running).map(|behavior| behavior.debug_string()).collect();
        format!("GateBehavior: {running:?}")
    }
}
