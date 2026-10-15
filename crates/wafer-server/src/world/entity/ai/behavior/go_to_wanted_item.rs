use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;

pub fn create(speed_modifier: f32, interrupt_ongoing_walk: bool, max_dist_to_walk: i32) -> OneShot {
    create_with(|_, _| true, speed_modifier, interrupt_ongoing_walk, max_dist_to_walk)
}

pub fn create_with(predicate: impl Fn(&ServerLevel, EntityId) -> bool + 'static, speed_modifier: f32, interrupt_ongoing_walk: bool, max_dist_to_walk: i32) -> OneShot {
    behavior_builder::create(|i| {
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        if interrupt_ongoing_walk {
            i.registered(memory_module_type::WALK_TARGET);
        } else {
            i.absent(memory_module_type::WALK_TARGET);
        }
        let wanted_item = i.present(memory_module_type::NEAREST_VISIBLE_WANTED_ITEM);
        let cooldown = i.registered(memory_module_type::ITEM_PICKUP_COOLDOWN_TICKS);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let item = wanted_item.get(level, body);
            let Some(item_entity) = level.entity_ref(item) else { return false };
            let item_pos = item_entity.block_position();
            let max = max_dist_to_walk as f64;
            let closer = item_entity.position().distance_to_sqr(level.entity(body).position()) < max * max;
            if cooldown.try_get(level, body).is_none()
                && predicate(level, body)
                && closer
                && level.world_border.is_within_bounds_pos(item_pos)
                && level.mob(body).is_some_and(|mob| mob.can_pick_up_loot)
            {
                let target = WalkTarget::new(PositionTracker::Entity(EntityTracker::new(level, item, false)), speed_modifier, 0);
                let look = PositionTracker::Entity(EntityTracker::new(level, item, true));
                look_target.set(level, body, look);
                level.get_brain_mut(body).set_memory(memory_module_type::WALK_TARGET, target);
                true
            } else {
                false
            }
        }
    })
}
