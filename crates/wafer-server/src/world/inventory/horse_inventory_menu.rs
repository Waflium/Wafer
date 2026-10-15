use wafer_data::EquipmentSlot;

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use super::abstract_mount_inventory_menu::INVENTORY_ROWS;
use super::slot::{Slot, SlotContainer, SlotKind};
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;

impl AbstractContainerMenu {
    pub fn horse_inventory(container_id: i32, horse: EntityId, inventory_columns: usize, inventory_version: u32) -> AbstractContainerMenu {
        let kind = MenuKind::HorseInventory { mount: horse, inventory_columns, inventory_version };
        AbstractContainerMenu::new(container_id, kind, Some(ContainerRef::HorseInventory(horse)))
    }
}

pub fn add_mount_slots(menu: &mut AbstractContainerMenu, horse: EntityId, inventory_columns: usize) {
    let saddle_container = SlotContainer::Container(ContainerRef::MobEquipmentSlot(horse, EquipmentSlot::Saddle));
    menu.add_slot(Slot::new(saddle_container, 0, SlotKind::MountArmor(horse, EquipmentSlot::Saddle)));
    let armor_container = SlotContainer::Container(ContainerRef::MobEquipmentSlot(horse, EquipmentSlot::Body));
    menu.add_slot(Slot::new(armor_container, 0, SlotKind::MountArmor(horse, EquipmentSlot::Body)));
    if inventory_columns > 0 {
        for y in 0..INVENTORY_ROWS {
            for x in 0..inventory_columns {
                menu.add_slot(Slot::new(SlotContainer::Container(ContainerRef::HorseInventory(horse)), x + y * inventory_columns, SlotKind::Normal));
            }
        }
    }
}
