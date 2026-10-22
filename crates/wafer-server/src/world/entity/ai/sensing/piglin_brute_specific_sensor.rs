use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::memory::nearest_visible_living_entities::NearestVisibleLivingEntities;
use crate::world::entity::monster::piglin::piglin_ai;
use crate::world::entity::{EntityId, entity_types};

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase(), memory_module_type::NEAREST_VISIBLE_NEMESIS.erase(), memory_module_type::NEARBY_ADULT_PIGLINS.erase()]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let visible_living_entities = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned().unwrap_or_else(NearestVisibleLivingEntities::empty);
    let nemesis = visible_living_entities.find_closest(level, |level, entity| {
        let entity_type = level.entity(entity).entity_type;
        entity_type == entity_types::WITHER_SKELETON.id || entity_type == entity_types::WITHER.id
    });
    let adult_piglins = piglin_ai::find_nearby_adult_piglins(level, body);
    let brain = level.get_brain_mut(body);
    brain.set_memory_optional(memory_module_type::NEAREST_VISIBLE_NEMESIS, nemesis);
    brain.set_memory(memory_module_type::NEARBY_ADULT_PIGLINS, adult_piglins);
}
