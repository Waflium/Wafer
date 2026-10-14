use wafer_data::EquipmentSlot;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Default)]
pub struct RestrictSunGoal;

impl RestrictSunGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        level.is_bright_outside() && level.item_by_slot_ref(mob, EquipmentSlot::Head).is_none() && level.mob(mob).expect("a mob").ai.navigation.can_navigate_ground()
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").ai.navigation.set_avoid_sun(true);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").ai.navigation.set_avoid_sun(false);
    }
}
