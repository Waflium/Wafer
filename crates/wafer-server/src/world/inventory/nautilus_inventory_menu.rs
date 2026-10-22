use wafer_data::EquipmentSlot;

use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use super::slot::{Slot, SlotContainer, SlotKind};
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;

impl AbstractContainerMenu {
    pub fn nautilus_inventory(container_id: i32, nautilus: EntityId, inventory_version: u32) -> AbstractContainerMenu {
        let kind = MenuKind::NautilusInventory { mount: nautilus, inventory_version };
        AbstractContainerMenu::new(container_id, kind, Some(ContainerRef::MountInventory(nautilus)))
    }
}

pub fn add_mount_slots(menu: &mut AbstractContainerMenu, nautilus: EntityId) {
    let saddle_container = SlotContainer::Container(ContainerRef::MobEquipmentSlot(nautilus, EquipmentSlot::Saddle));
    menu.add_slot(Slot::new(saddle_container, 0, SlotKind::MountArmor(nautilus, EquipmentSlot::Saddle)));
    let armor_container = SlotContainer::Container(ContainerRef::MobEquipmentSlot(nautilus, EquipmentSlot::Body));
    menu.add_slot(Slot::new(armor_container, 0, SlotKind::MountArmor(nautilus, EquipmentSlot::Body)));
}
