use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::memory::memory_value::EntityValue;
use crate::world::entity::ai::memory::nearest_visible_living_entities::NearestVisibleLivingEntities;

pub fn requires(memory_to_set: MemoryModuleType<EntityValue>) -> Vec<MemoryModuleType<Erased>> {
    vec![memory_to_set.erase(), memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase()]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId, memory_to_set: MemoryModuleType<EntityValue>, is_matching_entity: fn(&mut ServerLevel, EntityId, EntityId) -> bool) {
    let nearest_entity = get_nearest_entity(level, body, is_matching_entity);
    level.get_brain_mut(body).set_memory_optional(memory_to_set, nearest_entity);
}

fn get_nearest_entity(level: &mut ServerLevel, body: EntityId, is_matching_entity: fn(&mut ServerLevel, EntityId, EntityId) -> bool) -> Option<EntityId> {
    let living_entities = get_visible_entities(level, body)?;
    living_entities.find_closest(level, |level, mob| is_matching_entity(level, body, mob))
}

fn get_visible_entities(level: &ServerLevel, body: EntityId) -> Option<NearestVisibleLivingEntities> {
    level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned()
}
