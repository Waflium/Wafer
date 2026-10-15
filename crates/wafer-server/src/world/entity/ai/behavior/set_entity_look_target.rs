use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::mob_category::MobCategory;

pub fn create_for_category(category: MobCategory, max_dist: f32) -> OneShot {
    create_with(move |level, mob| EntityType::by_id(level.entity(mob).entity_type).get_category() == category, max_dist)
}

pub fn create_for_type(entity_type: &'static EntityType, max_dist: f32) -> OneShot {
    create_with(move |level, mob| level.entity(mob).entity_type == entity_type.id, max_dist)
}

pub fn create_with(predicate: impl Fn(&ServerLevel, EntityId) -> bool + 'static, max_dist: f32) -> OneShot {
    let max_dist_sqr = max_dist * max_dist;
    behavior_builder::create(|i| {
        let look_target = i.absent(memory_module_type::LOOK_TARGET);
        let nearest_entities = i.present(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = nearest_entities.get(level, body).find_closest(level, |level, mob| {
                predicate(level, mob) && level.entity(mob).position().distance_to_sqr(level.entity(body).position()) <= max_dist_sqr as f64 && !level.has_passenger(body, mob)
            });
            let Some(target) = target else { return false };
            let tracker = EntityTracker::new(level, target, true);
            look_target.set(level, body, PositionTracker::Entity(tracker));
            true
        }
    })
}
