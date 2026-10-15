use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::entity_type::EntityType;

pub fn create(entity_type: &'static EntityType, interaction_range: i32) -> OneShot {
    let interaction_range_sqr = interaction_range * interaction_range;
    behavior_builder::create(|i| {
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let interaction_target = i.absent(memory_module_type::INTERACTION_TARGET);
        let nearest_entities = i.present(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let closest = nearest_entities.get(level, body).find_closest(level, |level, e| {
                level.entity(e).position().distance_to_sqr(level.entity(body).position()) <= interaction_range_sqr as f64 && level.entity(e).entity_type == entity_type.id
            });
            let Some(closest_entity) = closest else { return false };
            interaction_target.set(level, body, closest_entity);
            let tracker = EntityTracker::new(level, closest_entity, true);
            look_target.set(level, body, PositionTracker::Entity(tracker));
            true
        }
    })
}
