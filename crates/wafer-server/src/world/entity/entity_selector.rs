use super::EntityId;
use crate::server::level::ServerLevel;

pub fn entity_not_being_ridden(level: &ServerLevel, entity: EntityId) -> bool {
    level.is_alive(entity) && !level.is_vehicle(entity) && !level.is_passenger(entity)
}

pub fn no_creative_or_spectator(level: &ServerLevel, entity: EntityId) -> bool {
    !(level.is_player(entity) && (level.is_spectator(entity) || level.player(entity).game_mode.game_type.is_creative()))
}

pub fn not_riding(level: &ServerLevel, entity: EntityId, input: EntityId) -> bool {
    let mut input = input;
    while let Some(vehicle) = level.vehicle(input) {
        input = vehicle;
        if input == entity {
            return false;
        }
    }
    true
}
