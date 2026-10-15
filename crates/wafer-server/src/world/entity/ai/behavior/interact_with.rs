use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, MemoryModuleType};
use crate::world::entity::ai::memory::memory_value::EntityValue;
use crate::world::entity::ai::memory::walk_target::WalkTarget;
use crate::world::entity::entity_type::EntityType;

pub fn of(entity_type: &'static EntityType, interaction_range: i32, interaction_target: MemoryModuleType<EntityValue>, speed_modifier: f32, stop_distance: i32) -> OneShot {
    of_with(entity_type, interaction_range, |_, _| true, |_, _| true, interaction_target, speed_modifier, stop_distance)
}

pub fn of_with(
    entity_type: &'static EntityType,
    interaction_range: i32,
    self_filter: impl Fn(&ServerLevel, EntityId) -> bool + 'static,
    target_filter: impl Fn(&ServerLevel, EntityId) -> bool + Copy + 'static,
    interaction_target: MemoryModuleType<EntityValue>,
    speed_modifier: f32,
    stop_distance: i32,
) -> OneShot {
    let interaction_range_sqr = interaction_range * interaction_range;
    let is_target_valid = move |level: &ServerLevel, mob: EntityId| level.entity(mob).entity_type == entity_type.id && target_filter(level, mob);
    behavior_builder::create(|i| {
        let target = i.registered(interaction_target);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let walk_target = i.absent(memory_module_type::WALK_TARGET);
        let nearest_entities = i.present(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let entities = nearest_entities.get(level, body);
            if self_filter(level, body) && entities.contains_matching(level, |level, mob| is_target_valid(level, mob)) {
                let closest = entities.find_closest(level, |level, mob| {
                    level.entity(mob).position().distance_to_sqr(level.entity(body).position()) <= interaction_range_sqr as f64 && is_target_valid(level, mob)
                });
                if let Some(mob) = closest {
                    target.set(level, body, mob);
                    let look = PositionTracker::Entity(EntityTracker::new(level, mob, true));
                    look_target.set(level, body, look);
                    let walk = WalkTarget::new(PositionTracker::Entity(EntityTracker::new(level, mob, false)), speed_modifier, stop_distance);
                    walk_target.set(level, body, walk);
                }
                true
            } else {
                false
            }
        }
    })
}
