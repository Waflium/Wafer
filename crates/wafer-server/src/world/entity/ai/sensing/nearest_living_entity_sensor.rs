use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::memory::nearest_visible_living_entities::NearestVisibleLivingEntities;
use crate::world::entity::attributes::Attribute;

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let follow_range = level.attribute_value(body, Attribute::FollowRange);
    let bounding_box = level.entity(body).bounding_box().inflate(follow_range, follow_range, follow_range);
    let mut living_entities = level.get_living_entities(&bounding_box, |level, mob| mob != body && level.is_alive(mob));
    let position = level.entity(body).position();
    living_entities.sort_by(|&a, &b| position.distance_to_sqr(level.entity(a).position()).total_cmp(&position.distance_to_sqr(level.entity(b).position())));
    let nearest_visible_living_entities = NearestVisibleLivingEntities::new(body, living_entities.clone());
    let brain = level.get_brain_mut(body);
    brain.set_memory(memory_module_type::NEAREST_LIVING_ENTITIES, living_entities);
    brain.set_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES, nearest_visible_living_entities);
}

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::NEAREST_LIVING_ENTITIES.erase(), memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase()]
}
