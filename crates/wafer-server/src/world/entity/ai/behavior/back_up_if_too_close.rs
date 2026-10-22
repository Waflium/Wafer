use wafer_core::mth;

use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;

pub fn create(too_close_distance: i32, strafe_speed: f32) -> OneShot {
    behavior_builder::create(|i| {
        let _walk_target = i.absent(memory_module_type::WALK_TARGET);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let attack_target = i.present(memory_module_type::ATTACK_TARGET);
        let nearest_visible = i.present(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = attack_target.get(level, body);
            let close = level.entity(target).position().closer_than(level.entity(body).position(), too_close_distance as f64);
            if close && nearest_visible.get(level, body).contains(level, target) {
                let tracker = EntityTracker::new(level, target, true);
                look_target.set(level, body, PositionTracker::Entity(tracker));
                let mob = level.mob_mut(body).expect("a mob");
                mob.ai.move_control.strafe(-strafe_speed, 0.0);
                mob.entity.y_rot = mth::rotate_if_necessary(mob.entity.y_rot, mob.entity.y_head_rot, 0.0);
                true
            } else {
                false
            }
        }
    })
}
