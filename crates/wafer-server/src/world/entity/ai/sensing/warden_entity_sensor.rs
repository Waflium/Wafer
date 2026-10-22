use super::nearest_living_entity_sensor;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    let mut requires = nearest_living_entity_sensor::requires();
    requires.push(memory_module_type::NEAREST_ATTACKABLE.erase());
    requires
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    nearest_living_entity_sensor::do_tick(level, body);
    let nearest = get_closest(level, body, |level, entity| level.is_player(entity)).or_else(|| get_closest(level, body, |level, entity| !level.is_player(entity)));
    let brain = level.get_brain_mut(body);
    match nearest {
        Some(entity) => brain.set_memory(memory_module_type::NEAREST_ATTACKABLE, entity),
        None => brain.erase_memory(memory_module_type::NEAREST_ATTACKABLE),
    }
}

fn get_closest(level: &ServerLevel, body: EntityId, test: impl Fn(&ServerLevel, EntityId) -> bool) -> Option<EntityId> {
    let entities = level.get_brain(body).get_memory(memory_module_type::NEAREST_LIVING_ENTITIES)?;
    entities.iter().copied().find(|&entity| level.warden_can_target_entity(body, Some(entity)) && test(level, entity))
}
