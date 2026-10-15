use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::ai::memory::memory_module_type;

const SAFE_DISTANCE_FROM_DANGER: f64 = 36.0;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let hurt_by = i.registered(memory_module_type::HURT_BY);
        let hurt_by_entity = i.registered(memory_module_type::HURT_BY_ENTITY);
        let nearest_hostile = i.registered(memory_module_type::NEAREST_HOSTILE);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let feel_scared = hurt_by.try_get(level, body).is_some()
                || nearest_hostile.try_get(level, body).is_some()
                || hurt_by_entity
                    .try_get(level, body)
                    .is_some_and(|entity| level.entity_ref(entity).is_some_and(|e| e.position().distance_to_sqr(level.entity(body).position()) <= SAFE_DISTANCE_FROM_DANGER));
            if !feel_scared {
                hurt_by.erase(level, body);
                hurt_by_entity.erase(level, body);
                let (game_time, position) = (level.game_time, level.entity(body).position());
                Brain::update_activity_from_schedule(level, body, game_time, position);
            }
            true
        }
    })
}
