use super::{nearest_visible_living_entity_sensor, sensor};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::memory::memory_value::EntityValue;
use crate::world::entity::animal::frog::frog;

pub const TARGET_DETECTION_DISTANCE: f32 = 10.0;

fn is_matching_entity(level: &mut ServerLevel, body: EntityId, mob: EntityId) -> bool {
    if sensor::is_entity_attackable(level, body, mob) && frog::can_eat(level, mob) && !is_unreachable_attack_target(level, body, mob) {
        level.entity(mob).distance_to_sqr(level.entity(body)) < (TARGET_DETECTION_DISTANCE as f64) * (TARGET_DETECTION_DISTANCE as f64)
    } else {
        false
    }
}

fn is_unreachable_attack_target(level: &ServerLevel, body: EntityId, mob: EntityId) -> bool {
    let uuid = level.entity(mob).uuid;
    level.get_brain(body).get_memory(memory_module_type::UNREACHABLE_TONGUE_TARGETS).is_some_and(|targets| targets.contains(&uuid))
}

fn get_memory_to_set() -> MemoryModuleType<EntityValue> {
    memory_module_type::NEAREST_ATTACKABLE
}

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    let mut requires = nearest_visible_living_entity_sensor::requires(get_memory_to_set());
    requires.push(memory_module_type::UNREACHABLE_TONGUE_TARGETS.erase());
    requires
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    nearest_visible_living_entity_sensor::do_tick(level, body, get_memory_to_set(), is_matching_entity);
}
