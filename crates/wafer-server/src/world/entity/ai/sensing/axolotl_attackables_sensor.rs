use wafer_data::entity_type_tags;

use super::{nearest_visible_living_entity_sensor, sensor};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::ai::memory::memory_value::EntityValue;

pub const TARGET_DETECTION_DISTANCE: f32 = 8.0;

fn is_matching_entity(level: &mut ServerLevel, body: EntityId, mob: EntityId) -> bool {
    is_close(level, body, mob) && level.entity(mob).is_in_water() && (is_hostile_target(level, mob) || is_hunt_target(level, body, mob)) && sensor::is_entity_attackable(level, body, mob)
}

fn is_hunt_target(level: &ServerLevel, body: EntityId, mob: EntityId) -> bool {
    !level.get_brain(body).has_memory_value(memory_module_type::HAS_HUNTING_COOLDOWN) && entity_type_tags::AXOLOTL_HUNT_TARGETS.contains(level.entity(mob).entity_type as usize)
}

fn is_hostile_target(level: &ServerLevel, mob: EntityId) -> bool {
    entity_type_tags::AXOLOTL_ALWAYS_HOSTILES.contains(level.entity(mob).entity_type as usize)
}

fn is_close(level: &ServerLevel, body: EntityId, mob: EntityId) -> bool {
    level.entity(mob).distance_to_sqr(level.entity(body)) <= (TARGET_DETECTION_DISTANCE * TARGET_DETECTION_DISTANCE) as f64
}

fn get_memory_to_set() -> MemoryModuleType<EntityValue> {
    memory_module_type::NEAREST_ATTACKABLE
}

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    let mut requires = nearest_visible_living_entity_sensor::requires(get_memory_to_set());
    requires.push(memory_module_type::HAS_HUNTING_COOLDOWN.erase());
    requires
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    nearest_visible_living_entity_sensor::do_tick(level, body, get_memory_to_set(), is_matching_entity);
}
