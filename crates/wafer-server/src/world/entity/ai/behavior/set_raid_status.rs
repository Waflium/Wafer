use wafer_core::random::RandomSource;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::schedule::Activity;

pub fn create() -> OneShot {
    behavior_builder::create(|_| {
        |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.random.next_int_bounded(20) != 0 {
                return false;
            }
            let pos = level.entity(body).block_position();
            if let Some(nearby_raid) = level.get_raid_at(pos) {
                let raid_activity = if nearby_raid.has_first_wave_spawned() && !nearby_raid.is_between_waves() { Activity::RAID } else { Activity::PRE_RAID };
                let brain = level.get_brain_mut(body);
                brain.set_default_activity(raid_activity);
                brain.set_active_activity_if_possible(raid_activity);
            }
            true
        }
    })
}
