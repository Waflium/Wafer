use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type::{Erased, MemoryModuleType};

pub struct Absent;

pub struct Present;

pub struct Registered;

#[derive(Clone, Copy, Debug)]
pub enum MemoryCondition {
    Absent(MemoryModuleType<Erased>),
    Present(MemoryModuleType<Erased>),
    Registered(MemoryModuleType<Erased>),
}

impl MemoryCondition {
    pub fn memory(self) -> MemoryModuleType<Erased> {
        match self {
            MemoryCondition::Absent(memory) | MemoryCondition::Present(memory) | MemoryCondition::Registered(memory) => memory,
        }
    }

    pub fn create_accessor(self, brain: &Brain) -> bool {
        match brain.get_memory_internal_erased(self.memory()) {
            None => false,
            Some(value) => match self {
                MemoryCondition::Absent(_) => !value,
                MemoryCondition::Present(_) => value,
                MemoryCondition::Registered(_) => true,
            },
        }
    }
}
