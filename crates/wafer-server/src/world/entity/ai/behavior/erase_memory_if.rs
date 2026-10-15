use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::MemoryModuleType;
use crate::world::entity::ai::memory::memory_value::MemoryValueType;

pub fn create<U: MemoryValueType>(predicate: fn(&ServerLevel, EntityId) -> bool, memory_type: MemoryModuleType<U>) -> OneShot {
    behavior_builder::create(|i| {
        let memory = i.present(memory_type);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if predicate(level, body) {
                memory.erase(level, body);
                true
            } else {
                false
            }
        }
    })
}
