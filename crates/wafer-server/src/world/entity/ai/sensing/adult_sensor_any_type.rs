use wafer_data::entity_type_tags;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::nearest_visible_living_entities::NearestVisibleLivingEntities;

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    if let Some(living_entities) = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned() {
        set_nearest_visible_adult(level, body, &living_entities);
    }
}

fn set_nearest_visible_adult(level: &mut ServerLevel, body: EntityId, visible_living_entities: &NearestVisibleLivingEntities) {
    let adult = visible_living_entities.find_closest(level, |level, entity| entity_type_tags::FOLLOWABLE_FRIENDLY_MOBS.contains(level.entity(entity).entity_type as usize) && !level.is_baby(entity));
    level.get_brain_mut(body).set_memory_optional(memory_module_type::NEAREST_VISIBLE_ADULT, adult);
}
