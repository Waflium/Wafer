use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;

pub type StartAttackingCondition = fn(&ServerLevel, EntityId) -> bool;

pub type TargetFinder = fn(&mut ServerLevel, EntityId) -> Option<EntityId>;

pub fn create(target_finder_function: TargetFinder) -> OneShot {
    create_with(|_, _| true, target_finder_function)
}

pub fn create_with(can_attack_predicate: StartAttackingCondition, target_finder_function: TargetFinder) -> OneShot {
    behavior_builder::create(|i| {
        let attack_target = i.absent(memory_module_type::ATTACK_TARGET);
        let cant_reach_since = i.registered(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if !can_attack_predicate(level, body) {
                return false;
            }
            let Some(target_entity) = target_finder_function(level, body) else { return false };
            if level.entity_ref(target_entity).is_none() || !level.can_attack(body, target_entity) {
                return false;
            }
            attack_target.set(level, body, target_entity);
            cant_reach_since.erase(level, body);
            true
        }
    })
}
