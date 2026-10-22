use wafer_data::EquipmentSlot;

use super::piglin_ai;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::declarative::behavior_builder;
use crate::world::entity::ai::behavior::one_shot::OneShot;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::item::component as dc;

pub fn create() -> OneShot {
    behavior_builder::create(|i| {
        let _admiring = i.absent(memory_module_type::ADMIRING_ITEM);
        move |level: &mut ServerLevel, body: EntityId, _timestamp: i64| {
            if level.item_by_slot_ref(body, EquipmentSlot::Offhand).is_some_and(|off_hand_item| !off_hand_item.has(dc::BLOCKS_ATTACKS)) {
                piglin_ai::stop_holding_off_hand_item(level, body, true);
                true
            } else {
                false
            }
        }
    })
}
