use super::piglin_ai;
use crate::server::level::ServerLevel;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::{EntityId, LevelEntity};

pub fn create(admire_duration: i32) -> OneShot {
    behavior_builder::create(|i| {
        let nearest_item = i.present(memory_module_type::NEAREST_VISIBLE_WANTED_ITEM);
        let admiring = i.absent(memory_module_type::ADMIRING_ITEM);
        let _admiring_disabled = i.absent(memory_module_type::ADMIRING_DISABLED);
        let _walk_disabled = i.absent(memory_module_type::DISABLE_WALK_TO_ADMIRE_ITEM);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            let item_entity = nearest_item.get(level, body);
            let Some(LevelEntity::Item(item)) = level.find_entity(item_entity) else { return false };
            if !piglin_ai::is_loved_item(&item.item) {
                return false;
            }
            admiring.set_with_expiry(level, body, true, admire_duration as i64);
            true
        }
    })
}
