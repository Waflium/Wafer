use std::marker::PhantomData;

use super::memory_condition::Present;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::MemoryModuleType;
use crate::world::entity::ai::memory::memory_value::MemoryValueType;

pub struct MemoryAccessor<F, U> {
    memory_type: MemoryModuleType<U>,
    _condition: PhantomData<fn() -> F>,
}

impl<F, U> Clone for MemoryAccessor<F, U> {
    fn clone(&self) -> MemoryAccessor<F, U> {
        *self
    }
}

impl<F, U> Copy for MemoryAccessor<F, U> {}

impl<F, U: MemoryValueType> MemoryAccessor<F, U> {
    pub fn new(memory_type: MemoryModuleType<U>) -> MemoryAccessor<F, U> {
        MemoryAccessor { memory_type, _condition: PhantomData }
    }

    pub fn set(self, level: &mut ServerLevel, body: EntityId, value: U::Value) {
        level.get_brain_mut(body).set_memory(self.memory_type, value);
    }

    pub fn set_or_erase(self, level: &mut ServerLevel, body: EntityId, value: Option<U::Value>) {
        level.get_brain_mut(body).set_memory_optional(self.memory_type, value);
    }
}

impl<U: MemoryValueType> MemoryAccessor<Present, U> {
    pub fn get(self, level: &ServerLevel, body: EntityId) -> U::Value {
        level.get_brain(body).get_memory(self.memory_type).cloned().expect("a present memory")
    }
}
