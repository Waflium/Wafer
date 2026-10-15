use super::behavior_utils;
use super::declarative::behavior_builder;
use super::entity_tracker::EntityTracker;
use super::one_shot::OneShot;
use super::position_tracker::PositionTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::ai::memory::walk_target::WalkTarget;

const PROJECTILE_ATTACK_RANGE_BUFFER: i32 = 1;

pub fn create_with(speed_modifier: impl Fn(&ServerLevel, EntityId) -> f32 + 'static) -> OneShot {
    behavior_builder::create(|i| {
        let walk_target = i.registered(memory_module_type::WALK_TARGET);
        let look_target = i.registered(memory_module_type::LOOK_TARGET);
        let attack_target = i.present(memory_module_type::ATTACK_TARGET);
        let nearest_entities = i.registered(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let to_attack = attack_target.get(level, body);
            let entities = nearest_entities.try_get(level, body);
            if entities.is_some_and(|entities| entities.contains(level, to_attack)) && behavior_utils::is_within_attack_range(level, body, to_attack, PROJECTILE_ATTACK_RANGE_BUFFER) {
                walk_target.erase(level, body);
            } else {
                let tracker = EntityTracker::new(level, to_attack, true);
                look_target.set(level, body, PositionTracker::Entity(tracker));
                let target = WalkTarget::new(PositionTracker::Entity(EntityTracker::new(level, to_attack, false)), speed_modifier(level, body), 0);
                walk_target.set(level, body, target);
            }
            true
        }
    })
}
