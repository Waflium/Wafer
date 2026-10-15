use wafer_core::random::RandomSource;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::brain::Brain;
use crate::world::entity::schedule::Activity;

pub fn create() -> OneShot {
    behavior_builder::create(|_| {
        |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.random.next_int_bounded(20) != 0 {
                return false;
            }
            let pos = level.entity(body).block_position();
            if level.get_raid_at(pos).is_none_or(|raid| raid.is_stopped() || raid.is_loss()) {
                level.get_brain_mut(body).set_default_activity(Activity::IDLE);
                let (game_time, position) = (level.game_time, level.entity(body).position());
                Brain::update_activity_from_schedule(level, body, game_time, position);
            }
            true
        }
    })
}
