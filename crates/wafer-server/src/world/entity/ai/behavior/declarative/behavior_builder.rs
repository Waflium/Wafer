use super::memory_accessor::MemoryAccessor;
use super::memory_condition::{Absent, MemoryCondition, Present, Registered};
use super::trigger::Trigger;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type::MemoryModuleType;
use crate::world::entity::ai::memory::memory_value::MemoryValueType;

#[derive(Default)]
pub struct Instance {
    memories: Vec<MemoryCondition>,
}

impl Instance {
    pub fn registered<U: MemoryValueType>(&mut self, memory: MemoryModuleType<U>) -> MemoryAccessor<Registered, U> {
        self.memories.push(MemoryCondition::Registered(memory.erase()));
        MemoryAccessor::new(memory)
    }

    pub fn present<U: MemoryValueType>(&mut self, memory: MemoryModuleType<U>) -> MemoryAccessor<Present, U> {
        self.memories.push(MemoryCondition::Present(memory.erase()));
        MemoryAccessor::new(memory)
    }

    pub fn absent<U: MemoryValueType>(&mut self, memory: MemoryModuleType<U>) -> MemoryAccessor<Absent, U> {
        self.memories.push(MemoryCondition::Absent(memory.erase()));
        MemoryAccessor::new(memory)
    }
}

pub fn create<T: Trigger + 'static>(builder: impl FnOnce(&mut Instance) -> T) -> OneShot {
    let mut instance = Instance::default();
    let trigger = builder(&mut instance);
    OneShot::new(instance.memories, Box::new(trigger))
}
