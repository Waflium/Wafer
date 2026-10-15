use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::IS_IN_WATER.erase()]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    if level.entity(body).is_in_water() {
        level.get_brain_mut(body).set_memory(memory_module_type::IS_IN_WATER, ());
    } else {
        level.get_brain_mut(body).erase_memory(memory_module_type::IS_IN_WATER);
    }
}
