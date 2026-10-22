use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;

pub fn create(target_finder_function: fn(&ServerLevel, EntityId) -> Option<EntityId>) -> OneShot {
    behavior_builder::create(|i| {
        let roar_target = i.absent(memory_module_type::ROAR_TARGET);
        let _attack_target = i.absent(memory_module_type::ATTACK_TARGET);
        let cant_reach_since = i.registered(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let Some(target) = target_finder_function(level, body).filter(|&target| level.warden_can_target_entity(body, Some(target))) else { return false };
            roar_target.set(level, body, target);
            cant_reach_since.erase(level, body);
            true
        }
    })
}
