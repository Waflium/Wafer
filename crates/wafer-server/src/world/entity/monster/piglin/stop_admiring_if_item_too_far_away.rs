use wafer_data::EquipmentSlot;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;

pub fn create(max_distance_to_item: i32) -> OneShot {
    behavior_builder::create(|i| {
        let admiring = i.present(memory_module_type::ADMIRING_ITEM);
        let nearest = i.registered(memory_module_type::NEAREST_VISIBLE_WANTED_ITEM);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.item_by_slot_ref(body, EquipmentSlot::Offhand).is_some() {
                return false;
            }
            let nearest_visible_wanted_item = nearest.try_get(level, body);
            let max_distance = max_distance_to_item as f64;
            if nearest_visible_wanted_item.is_some_and(|item| level.entity(item).distance_to_sqr(level.entity(body)) < max_distance * max_distance) {
                return false;
            }
            admiring.erase(level, body);
            true
        }
    })
}
