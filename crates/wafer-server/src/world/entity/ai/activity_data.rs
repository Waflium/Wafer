use super::behavior::behavior_control::BehaviorControl;
use super::memory::memory_module_type::{Erased, MemoryModuleType};
use super::memory::memory_status::MemoryStatus;
use crate::world::entity::schedule::Activity;

pub struct ActivityData {
    pub activity_type: Activity,
    pub behavior_priority_pairs: Vec<(i32, Box<dyn BehaviorControl>)>,
    pub conditions: Vec<(MemoryModuleType<Erased>, MemoryStatus)>,
    pub memories_to_erase_when_stopped: Vec<MemoryModuleType<Erased>>,
}

impl ActivityData {
    pub fn create(activity: Activity, priority_of_first_behavior: i32, behavior_list: Vec<Box<dyn BehaviorControl>>) -> ActivityData {
        ActivityData::create_pairs(activity, ActivityData::create_priority_pairs(priority_of_first_behavior, behavior_list))
    }

    pub fn create_with_memory(
        activity: Activity,
        priority_of_first_behavior: i32,
        behavior_list: Vec<Box<dyn BehaviorControl>>,
        memory_that_must_have_value_and_will_be_erased_after: MemoryModuleType<Erased>,
    ) -> ActivityData {
        let conditions = vec![(memory_that_must_have_value_and_will_be_erased_after, MemoryStatus::ValuePresent)];
        let memories_to_erase_when_stopped = vec![memory_that_must_have_value_and_will_be_erased_after];
        ActivityData::new(activity, ActivityData::create_priority_pairs(priority_of_first_behavior, behavior_list), conditions, memories_to_erase_when_stopped)
    }

    pub fn create_pairs(activity: Activity, behavior_priority_pairs: Vec<(i32, Box<dyn BehaviorControl>)>) -> ActivityData {
        ActivityData::new(activity, behavior_priority_pairs, Vec::new(), Vec::new())
    }

    pub fn create_with_conditions(
        activity: Activity,
        priority_of_first_behavior: i32,
        behavior_list: Vec<Box<dyn BehaviorControl>>,
        conditions: Vec<(MemoryModuleType<Erased>, MemoryStatus)>,
    ) -> ActivityData {
        ActivityData::create_pairs_with_conditions(activity, ActivityData::create_priority_pairs(priority_of_first_behavior, behavior_list), conditions)
    }

    pub fn create_pairs_with_conditions(
        activity: Activity,
        behavior_priority_pairs: Vec<(i32, Box<dyn BehaviorControl>)>,
        conditions: Vec<(MemoryModuleType<Erased>, MemoryStatus)>,
    ) -> ActivityData {
        ActivityData::new(activity, behavior_priority_pairs, conditions, Vec::new())
    }

    pub fn new(
        activity: Activity,
        behavior_priority_pairs: Vec<(i32, Box<dyn BehaviorControl>)>,
        conditions: Vec<(MemoryModuleType<Erased>, MemoryStatus)>,
        memories_to_erase_when_stopped: Vec<MemoryModuleType<Erased>>,
    ) -> ActivityData {
        ActivityData { activity_type: activity, behavior_priority_pairs, conditions, memories_to_erase_when_stopped }
    }

    pub fn create_priority_pairs(priority_of_first_behavior: i32, behavior_list: Vec<Box<dyn BehaviorControl>>) -> Vec<(i32, Box<dyn BehaviorControl>)> {
        behavior_list.into_iter().enumerate().map(|(i, behavior)| (priority_of_first_behavior + i as i32, behavior)).collect()
    }
}
