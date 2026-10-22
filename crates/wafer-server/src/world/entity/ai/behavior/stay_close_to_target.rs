use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub fn create(
    target_position_getter: fn(&mut ServerLevel, EntityId) -> Option<PositionTracker>,
    should_run_predicate: fn(&ServerLevel, EntityId) -> bool,
    close_enough: i32,
    too_far: i32,
    speed_modifier: f32,
) -> OneShot {
    behavior_builder::create(move |i| {
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let Some(target) = target_position_getter(level, body) else { return false };
            if !should_run_predicate(level, body) {
                return false;
            }
            if level.entity(body).position().closer_than(target.current_position(level), too_far as f64) {
                return false;
            }
            look_target.set(level, body, target.clone());
            walk_target.set(level, body, WalkTarget::new(target, speed_modifier, close_enough));
            true
        }
    })
}
