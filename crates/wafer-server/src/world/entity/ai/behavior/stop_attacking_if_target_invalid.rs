use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;

const TIMEOUT_TO_GET_WITHIN_ATTACK_RANGE: i64 = 200;

pub type StopAttackCondition = fn(&mut ServerLevel, EntityId, EntityId) -> bool;

pub type TargetErasedCallback = fn(&mut ServerLevel, EntityId, EntityId);

pub fn create() -> OneShot {
    create_with(|_, _, _| false, |_, _, _| {}, true)
}

pub fn create_when(stop_attacking_when: StopAttackCondition) -> OneShot {
    create_with(stop_attacking_when, |_, _, _| {}, true)
}

pub fn create_with_callback(on_target_erased: TargetErasedCallback) -> OneShot {
    create_with(|_, _, _| false, on_target_erased, true)
}

pub fn create_with(stop_attacking_when: StopAttackCondition, on_target_erased: TargetErasedCallback, can_grow_tired_of_trying_to_reach_target: bool) -> OneShot {
    behavior_builder::create(|i| {
        let attack_target = i.present(memory_module_type::ATTACK_TARGET);
        let cant_reach_since = i.registered(memory_module_type::CANT_REACH_WALK_TARGET_SINCE);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = attack_target.get(level, body);
            if level.entity_ref(target).is_some()
                && level.can_attack(body, target)
                && (!can_grow_tired_of_trying_to_reach_target || !is_tired_of_trying_to_reach_target(level, cant_reach_since.try_get(level, body)))
                && level.is_alive(target)
                && !stop_attacking_when(level, body, target)
            {
                return true;
            }
            on_target_erased(level, body, target);
            attack_target.erase(level, body);
            true
        }
    })
}

fn is_tired_of_trying_to_reach_target(level: &ServerLevel, cant_reach_since: Option<i64>) -> bool {
    cant_reach_since.is_some_and(|cant_reach_since| level.game_time - cant_reach_since > TIMEOUT_TO_GET_WITHIN_ATTACK_RANGE)
}
