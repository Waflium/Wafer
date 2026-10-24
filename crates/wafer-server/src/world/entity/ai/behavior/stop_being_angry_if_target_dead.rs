use wafer_data::game_rule;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::{EntityId, entity_types};

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let angry_at = i.present(memory_module_type::ANGRY_AT);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let anger_target = level.entity_manager.visible_entity_storage.get(angry_at.get(level, body)).filter(|&entity| level.is_living(entity));
            if let Some(anger_target) = anger_target
                && level.is_dead_or_dying(anger_target)
                && (level.entity(anger_target).entity_type != entity_types::PLAYER.id || level.game_rules.get_bool(game_rule::FORGIVE_DEAD_PLAYERS))
            {
                angry_at.erase(level, body);
            }
            true
        }
    })
}
