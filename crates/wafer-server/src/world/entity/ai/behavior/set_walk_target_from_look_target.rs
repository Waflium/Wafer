use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub fn create(speed_modifier: f32, close_enough_distance: i32) -> OneShot {
    create_with(|_, _| true, move |_, _| speed_modifier, close_enough_distance)
}

pub fn create_with(
    can_set_walk_target_predicate: impl Fn(&ServerLevel, EntityId) -> bool + 'static,
    speed_modifier: impl Fn(&ServerLevel, EntityId) -> f32 + 'static,
    close_enough_distance: i32,
) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        let look_target = i.present(memory_module_type::LOOK_TARGET);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if !can_set_walk_target_predicate(level, body) {
                return false;
            }
            let target = WalkTarget::new(look_target.get(level, body), speed_modifier(level, body), close_enough_distance);
            walk_target.set(level, body, target);
            true
        }
    })
}
