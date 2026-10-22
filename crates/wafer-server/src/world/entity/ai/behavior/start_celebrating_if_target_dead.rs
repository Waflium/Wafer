use wafer_data::game_rule;

use super::declarative::behavior_builder;
use super::one_shot::OneShot;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::{EntityId, entity_types};

pub fn create(celebrate_duration: i32, dance_predicate: fn(&ServerLevel, EntityId, EntityId) -> bool) -> OneShot {
    behavior_builder::create(|i| {
        let attack_target = i.present(memory_module_type::ATTACK_TARGET);
        let angry_at = i.registered(memory_module_type::ANGRY_AT);
        let celebrate_at = i.absent(memory_module_type::CELEBRATE_LOCATION);
        let dancing = i.registered(memory_module_type::DANCING);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = attack_target.get(level, body);
            if !level.is_dead_or_dying(target) {
                return false;
            }
            if dance_predicate(level, body, target) {
                dancing.set_with_expiry(level, body, true, celebrate_duration as i64);
            }
            let target_pos = level.entity(target).block_position();
            celebrate_at.set_with_expiry(level, body, target_pos, celebrate_duration as i64);
            if level.entity(target).entity_type != entity_types::PLAYER.id || level.game_rules.get_bool(game_rule::FORGIVE_DEAD_PLAYERS) {
                attack_target.erase(level, body);
                angry_at.erase(level, body);
            }
            true
        }
    })
}
