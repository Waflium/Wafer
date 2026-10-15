use super::sensor;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type::{self, Erased, MemoryModuleType};
use crate::world::entity::attributes::Attribute;

pub fn requires() -> Vec<MemoryModuleType<Erased>> {
    vec![
        memory_module_type::NEAREST_PLAYERS.erase(),
        memory_module_type::NEAREST_VISIBLE_PLAYER.erase(),
        memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYER.erase(),
        memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYERS.erase(),
    ]
}

pub fn do_tick(level: &mut ServerLevel, body: EntityId) {
    let follow_distance = get_follow_distance(level, body);
    let position = level.entity(body).position();
    let mut players: Vec<EntityId> = level
        .level_players()
        .into_iter()
        .filter(|&player| !level.is_spectator(player) && position.distance_to_sqr(level.entity(player).position()) < follow_distance * follow_distance)
        .collect();
    players.sort_by(|&a, &b| position.distance_to_sqr(level.entity(a).position()).total_cmp(&position.distance_to_sqr(level.entity(b).position())));
    level.get_brain_mut(body).set_memory(memory_module_type::NEAREST_PLAYERS, players.clone());
    let visible_players: Vec<EntityId> = players.into_iter().filter(|&player| sensor::is_entity_targetable(level, body, player)).collect();
    level.get_brain_mut(body).set_memory_optional(memory_module_type::NEAREST_VISIBLE_PLAYER, visible_players.first().copied());
    let visible_attackable_players: Vec<EntityId> = visible_players.into_iter().filter(|&player| sensor::is_entity_attackable(level, body, player)).collect();
    let brain = level.get_brain_mut(body);
    brain.set_memory(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYERS, visible_attackable_players.clone());
    brain.set_memory_optional(memory_module_type::NEAREST_VISIBLE_ATTACKABLE_PLAYER, visible_attackable_players.first().copied());
}

fn get_follow_distance(level: &ServerLevel, body: EntityId) -> f64 {
    level.attribute_value(body, Attribute::FollowRange)
}
