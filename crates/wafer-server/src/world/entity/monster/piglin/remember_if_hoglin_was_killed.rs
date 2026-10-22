use super::piglin_ai;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::{EntityId, entity_types};

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let attack_target = i.present(memory_module_type::ATTACK_TARGET);
        let hunted_recently = i.registered(memory_module_type::HUNTED_RECENTLY);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let target = attack_target.get(level, body);
            if level.entity(target).entity_type == entity_types::HOGLIN.id && level.is_dead_or_dying(target) {
                let duration = piglin_ai::TIME_BETWEEN_HUNTS.sample(&mut level.random);
                hunted_recently.set_with_expiry(level, body, true, duration as i64);
            }
            true
        }
    })
}
