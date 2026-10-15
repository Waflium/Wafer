use super::behavior_control::BehaviorControl;
use super::gate_behavior::{GateBehavior, OrderPolicy, RunningPolicy};
use crate::world::entity::ai::memory::memory_module_type::{Erased, MemoryModuleType};
use crate::world::entity::ai::memory::memory_status::MemoryStatus;

pub fn new(weighted_behaviors: Vec<(Box<dyn BehaviorControl>, i32)>) -> GateBehavior {
    with_entry_condition(Vec::new(), weighted_behaviors)
}

pub fn with_entry_condition(entry_condition: Vec<(MemoryModuleType<Erased>, MemoryStatus)>, weighted_behaviors: Vec<(Box<dyn BehaviorControl>, i32)>) -> GateBehavior {
    GateBehavior::new(entry_condition, Vec::new(), OrderPolicy::Shuffled, RunningPolicy::RunOne, weighted_behaviors)
}
