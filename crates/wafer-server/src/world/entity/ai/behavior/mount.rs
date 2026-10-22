use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub fn create(speed_modifier: f32) -> OneShot {
    behavior_builder::create(|i| {
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        let ride_target = i.present(memory_module_type::RIDE_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.entity(body).vehicle.is_some() {
                return false;
            }
            let ridable_entity = ride_target.get(level, body);
            if level.entity(ridable_entity).position().closer_than(level.entity(body).position(), 1.0) {
                level.start_riding(body, ridable_entity, false, true);
            } else {
                let tracker = EntityTracker::new(level, ridable_entity, true);
                look_target.set(level, body, PositionTracker::Entity(tracker));
                let tracker = EntityTracker::new(level, ridable_entity, false);
                walk_target.set(level, body, WalkTarget::new(PositionTracker::Entity(tracker), speed_modifier, 1));
            }
            true
        }
    })
}
