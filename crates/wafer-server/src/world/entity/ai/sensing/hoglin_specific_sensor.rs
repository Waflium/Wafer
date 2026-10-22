use wafer_core::BlockPos;
use wafer_data::block_tags;

use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::memory::nearest_visible_living_entities::NearestVisibleLivingEntities;
use crate::world::entity::{EntityId, entity_types};
use crate::world::level::BlockGetter;

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![
        memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase(),
        memory_module_type::NEAREST_REPELLENT.erase(),
        memory_module_type::NEAREST_VISIBLE_ADULT_PIGLIN.erase(),
        memory_module_type::NEAREST_VISIBLE_ADULT_HOGLINS.erase(),
        memory_module_type::VISIBLE_ADULT_PIGLIN_COUNT.erase(),
        memory_module_type::VISIBLE_ADULT_HOGLIN_COUNT.erase(),
    ]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let nearest_repellent = find_nearest_repellent(level, body);
    level.get_brain_mut(body).set_memory_optional(memory_module_type::NEAREST_REPELLENT, nearest_repellent);
    let mut adult_piglin = None;
    let mut adult_piglin_count = 0;
    let mut adult_hoglins = Vec::new();
    let visible_living_entities = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned().unwrap_or_else(NearestVisibleLivingEntities::empty);
    let is_piglin = |level: &ServerLevel, entity: EntityId| level.entity(entity).entity_type == entity_types::PIGLIN.id;
    let is_hoglin = |level: &ServerLevel, entity: EntityId| level.entity(entity).entity_type == entity_types::HOGLIN.id;
    for entity in visible_living_entities.find_all(level, |level, entity| !level.is_baby(entity) && (is_piglin(level, entity) || is_hoglin(level, entity))) {
        if is_piglin(level, entity) {
            adult_piglin_count += 1;
            if adult_piglin.is_none() {
                adult_piglin = Some(entity);
            }
        }
        if is_hoglin(level, entity) {
            adult_hoglins.push(entity);
        }
    }
    let brain = level.get_brain_mut(body);
    brain.set_memory_optional(memory_module_type::NEAREST_VISIBLE_ADULT_PIGLIN, adult_piglin);
    let adult_hoglin_count = adult_hoglins.len() as i32;
    brain.set_memory(memory_module_type::NEAREST_VISIBLE_ADULT_HOGLINS, adult_hoglins);
    brain.set_memory(memory_module_type::VISIBLE_ADULT_PIGLIN_COUNT, adult_piglin_count);
    brain.set_memory(memory_module_type::VISIBLE_ADULT_HOGLIN_COUNT, adult_hoglin_count);
}

fn find_nearest_repellent(level: &mut ServerLevel, body: EntityId) -> Option<BlockPos> {
    let origin = level.entity(body).block_position();
    BlockPos::within_box_by_manhattan_distance(origin, 8, 4, 8).find(|&pos| level.block_state(pos).is(&block_tags::HOGLIN_REPELLENTS))
}
