use super::memory_accessor::MemoryAccessor;
use super::memory_condition::{Absent, MemoryCondition, Present, Registered};
use super::trigger::Trigger;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
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

pub fn sequence(mut first: impl Trigger + 'static, mut second: OneShot) -> OneShot {
    let required_memories = crate::world::entity::ai::behavior::behavior_control::BehaviorControl::get_required_memories(&second);
    OneShot::with_required_memories(
        Vec::new(),
        required_memories,
        Box::new(move |level: &mut crate::server::level::ServerLevel, body: crate::world::entity::EntityId, timestamp: i64| {
            first.trigger(level, body, timestamp) && second.trigger(level, body, timestamp)
        }),
    )
}

pub fn create<T: Trigger + 'static>(builder: impl FnOnce(&mut Instance) -> T) -> OneShot {
    let mut instance = Instance::default();
    let trigger = builder(&mut instance);
    OneShot::new(instance.memories, Box::new(trigger))
}

pub fn trigger_if(predicate: impl Fn(&ServerLevel, EntityId) -> bool + 'static) -> OneShot {
    create(|_| move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| predicate(level, body))
}
