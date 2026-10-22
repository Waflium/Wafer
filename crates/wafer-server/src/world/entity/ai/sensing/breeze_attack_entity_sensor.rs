use super::{nearest_living_entity_sensor, sensor};
use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::{EntityId, entity_selector};

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    let mut requires = nearest_living_entity_sensor::requires();
    requires.push(memory_module_type::NEAREST_ATTACKABLE.erase());
    requires
}

pub fn do_tick(level: &mut ServerLevel, breeze: EntityId) {
    nearest_living_entity_sensor::do_tick(level, breeze);
    let living_entities = level.get_brain(breeze).get_memory(memory_module_type::NEAREST_LIVING_ENTITIES).cloned().unwrap_or_default();
    let nearest_attackable = living_entities.into_iter().find(|&entity| entity_selector::no_creative_or_spectator(level, entity) && sensor::is_entity_attackable(level, breeze, entity));
    let brain = level.get_brain_mut(breeze);
    match nearest_attackable {
        Some(entity) => brain.set_memory(memory_module_type::NEAREST_ATTACKABLE, entity),
        None => brain.erase_memory(memory_module_type::NEAREST_ATTACKABLE),
    }
}
