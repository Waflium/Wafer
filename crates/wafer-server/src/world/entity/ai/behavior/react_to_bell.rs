use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::schedule::Activity;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        i.present(memory_module_type::HEARD_BELL_TIME);
        |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let pos = level.entity(body).block_position();
            if level.get_raid_at(pos).is_none() {
                level.get_brain_mut(body).set_active_activity_if_possible(Activity::HIDE);
            }
            true
        }
    })
}
