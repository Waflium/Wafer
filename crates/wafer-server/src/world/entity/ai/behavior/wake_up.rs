use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::schedule::Activity;

pub fn create() -> OneShot {
    behavior_builder::create(|_| {
        |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if !level.get_brain(body).is_active(Activity::REST) && level.is_sleeping(body) {
                level.stop_sleeping(body);
                true
            } else {
                false
            }
        }
    })
}
