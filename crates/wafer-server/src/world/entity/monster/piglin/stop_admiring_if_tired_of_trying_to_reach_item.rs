use wafer_data::EquipmentSlot;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;

pub fn create(max_time_to_reach_item: i32, disable_time: i32) -> OneShot {
    behavior_builder::create(|i| {
        let admiring = i.present(memory_module_type::ADMIRING_ITEM);
        let _nearest_visible = i.present(memory_module_type::NEAREST_VISIBLE_WANTED_ITEM);
        let time = i.registered(memory_module_type::TIME_TRYING_TO_REACH_ADMIRE_ITEM);
        let disable_walk = i.registered(memory_module_type::DISABLE_WALK_TO_ADMIRE_ITEM);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.item_by_slot_ref(body, EquipmentSlot::Offhand).is_some() {
                return false;
            }
            match time.try_get(level, body) {
                None => time.set(level, body, 0),
                Some(time_trying_to_reach) if time_trying_to_reach > max_time_to_reach_item => {
                    admiring.erase(level, body);
                    time.erase(level, body);
                    disable_walk.set_with_expiry(level, body, true, disable_time as i64);
                }
                Some(time_trying_to_reach) => time.set(level, body, time_trying_to_reach + 1),
            }
            true
        }
    })
}
