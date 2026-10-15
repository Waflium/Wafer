use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::brain::Brain;

pub fn create() -> OneShot {
    behavior_builder::create(|_| {
        |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let (game_time, position) = (level.game_time, level.entity(body).position());
            Brain::update_activity_from_schedule(level, body, game_time, position);
            true
        }
    })
}
