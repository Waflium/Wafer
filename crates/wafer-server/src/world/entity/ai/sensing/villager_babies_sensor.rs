use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::memory::nearest_visible_living_entities::NearestVisibleLivingEntities;
use crate::world::entity::{EntityId, entity_types};

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::VISIBLE_VILLAGER_BABIES.erase()]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let babies = get_nearest_villager_babies(level, body);
    level.get_brain_mut(body).set_memory(memory_module_type::VISIBLE_VILLAGER_BABIES, babies);
}

fn get_nearest_villager_babies(level: &mut ServerLevel, body: EntityId) -> Vec<EntityId> {
    get_visible_entities(level, body).find_all(level, |level, entity| is_villager_baby(level, entity))
}

fn is_villager_baby(level: &ServerLevel, entity: EntityId) -> bool {
    level.entity(entity).entity_type as i32 == entity_types::VILLAGER.id && level.is_baby(entity)
}

fn get_visible_entities(level: &ServerLevel, body: EntityId) -> NearestVisibleLivingEntities {
    level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned().unwrap_or_else(NearestVisibleLivingEntities::empty)
}
