use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::HURT_BY.erase(), memory_module_type::HURT_BY_ENTITY.erase()]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let damage_source = level.get_last_damage_source(body).copied();
    match damage_source {
        Some(damage_source) => {
            let causing_entity = damage_source.causing_entity;
            let living_entity = causing_entity.filter(|&entity| level.is_living(entity));
            let brain = level.get_brain_mut(body);
            brain.set_memory(memory_module_type::HURT_BY, damage_source);
            if let Some(living_entity) = living_entity {
                brain.set_memory(memory_module_type::HURT_BY_ENTITY, living_entity);
            } else if causing_entity.is_some_and(|entity| level.entity_ref(entity).is_none()) {
                level.get_brain_mut(body).erase_memory(memory_module_type::HURT_BY_ENTITY);
            }
        }
        None => level.get_brain_mut(body).erase_memory(memory_module_type::HURT_BY),
    }
    if let Some(hurt_by_entity) = level.get_brain(body).get_memory(memory_module_type::HURT_BY_ENTITY).copied()
        && (!level.is_alive(hurt_by_entity) || !level.is_in_level(hurt_by_entity))
    {
        level.get_brain_mut(body).erase_memory(memory_module_type::HURT_BY_ENTITY);
    }
}
