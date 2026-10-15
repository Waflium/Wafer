use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::{EntityId, entity_types};

fn acceptable_distance_from_hostiles(entity_type: i32) -> Option<f32> {
    let distance = match entity_type {
        t if t == entity_types::DROWNED.id => 8.0,
        t if t == entity_types::EVOKER.id => 12.0,
        t if t == entity_types::HUSK.id => 8.0,
        t if t == entity_types::ILLUSIONER.id => 12.0,
        t if t == entity_types::PILLAGER.id => 15.0,
        t if t == entity_types::RAVAGER.id => 12.0,
        t if t == entity_types::VEX.id => 8.0,
        t if t == entity_types::VINDICATOR.id => 10.0,
        t if t == entity_types::ZOGLIN.id => 10.0,
        t if t == entity_types::ZOMBIE.id => 8.0,
        t if t == entity_types::ZOMBIE_VILLAGER.id => 8.0,
        _ => return None,
    };
    Some(distance)
}

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![memory_module_type::NEAREST_HOSTILE.erase(), memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES.erase()]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let nearest = level
        .get_brain(body)
        .get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES)
        .cloned()
        .and_then(|living_entities| living_entities.find_closest(level, |level, mob| is_matching_entity(level, body, mob)));
    level.get_brain_mut(body).set_memory_optional(memory_module_type::NEAREST_HOSTILE, nearest);
}

fn is_matching_entity(level: &ServerLevel, body: EntityId, mob: EntityId) -> bool {
    is_hostile(level, mob) && is_close(level, body, mob)
}

fn is_close(level: &ServerLevel, body: EntityId, mob: EntityId) -> bool {
    let dist_threshold = acceptable_distance_from_hostiles(level.entity(mob).entity_type as i32).expect("a hostile");
    level.entity(mob).position().distance_to_sqr(level.entity(body).position()) <= (dist_threshold * dist_threshold) as f64
}

fn is_hostile(level: &ServerLevel, entity: EntityId) -> bool {
    acceptable_distance_from_hostiles(level.entity(entity).entity_type as i32).is_some()
}
