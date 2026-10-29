use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::{EntityId, entity_types};

pub const GOLEM_SCAN_RATE: i32 = 200;
const MEMORY_TIME_TO_LIVE: i64 = 599;

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    check_for_nearby_golem(level, body);
}

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::NEAREST_LIVING_ENTITIES.erase(), memory_module_type::GOLEM_DETECTED_RECENTLY.erase()]
}

pub fn check_for_nearby_golem(level: &mut ServerLevel, body: EntityId) {
    if let Some(living_entities) = level.get_brain(body).get_memory(memory_module_type::NEAREST_LIVING_ENTITIES) {
        let golem_present = living_entities.iter().any(|&entity| level.entity_ref_or_tombstone(entity).is_some_and(|e| e.entity_type as i32 == entity_types::IRON_GOLEM.id));
        if golem_present {
            golem_detected(level, body);
        }
    }
}

pub fn golem_detected(level: &mut ServerLevel, body: EntityId) {
    level.get_brain_mut(body).set_memory_with_expiry(memory_module_type::GOLEM_DETECTED_RECENTLY, true, MEMORY_TIME_TO_LIVE);
}
