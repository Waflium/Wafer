use wafer_core::random::RandomSource;
use wafer_data::EquipmentSlot;

use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;

pub struct UseItemGoal {
    item: ItemStack,
    can_use_selector: fn(&ServerLevel, EntityId) -> bool,
    finish_using_sound: Option<&'static str>,
}

impl UseItemGoal {
    pub fn new(item: ItemStack, finish_using_sound: Option<&'static str>, can_use_selector: fn(&ServerLevel, EntityId) -> bool) -> UseItemGoal {
        UseItemGoal { item, can_use_selector, finish_using_sound }
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        (self.can_use_selector)(level, mob)
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.is_using_item(mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.living_set_item_slot(mob, EquipmentSlot::Mainhand, self.item.copy());
        level.start_using_item(mob, InteractionHand::MainHand);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.living_set_item_slot(mob, EquipmentSlot::Mainhand, ItemStack::empty());
        if let Some(finish_using_sound) = self.finish_using_sound {
            let pitch = level.entity_mut(mob).random.next_float() * 0.2 + 0.9;
            level.entity_play_sound(mob, sound_event(finish_using_sound), 1.0, pitch);
        }
    }
}
