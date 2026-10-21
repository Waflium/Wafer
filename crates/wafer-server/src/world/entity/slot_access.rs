use wafer_data::EquipmentSlot;

use super::vehicle::boat::abstract_chest_boat;
use super::{EntityId, LevelEntity};
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::item::{ItemStack, inventory_slot};

pub trait SlotAccess {
    fn get(&mut self, level: &mut ServerLevel) -> ItemStack;

    fn set(&mut self, level: &mut ServerLevel, item_stack: ItemStack) -> bool;

    fn modify(&mut self, level: &mut ServerLevel, modifier: &mut dyn FnMut(&mut ServerLevel, &mut ItemStack));
}

#[derive(Clone, Debug)]
pub enum LevelSlot {
    Equipment { entity: EntityId, slot: EquipmentSlot },
    Carried(EntityId),
    Crafting(EntityId, usize),
    Inventory(EntityId, usize),
    EnderChest(EntityId, usize),
    VillagerInventory(EntityId, usize),
    PillagerInventory(EntityId, usize),
    Item(EntityId),
    Container(ContainerRef, usize),
    ChestedHorseChest(EntityId),
    Detached(ItemStack),
}

impl SlotAccess for LevelSlot {
    fn get(&mut self, level: &mut ServerLevel) -> ItemStack {
        level.level_slot_get(self)
    }

    fn set(&mut self, level: &mut ServerLevel, item_stack: ItemStack) -> bool {
        level.level_slot_set(self, item_stack)
    }

    fn modify(&mut self, level: &mut ServerLevel, modifier: &mut dyn FnMut(&mut ServerLevel, &mut ItemStack)) {
        level.level_slot_modify(self, modifier);
    }
}

fn get_equipment_slot(slot: i32) -> Option<EquipmentSlot> {
    Some(match slot {
        103 => EquipmentSlot::Head,
        102 => EquipmentSlot::Chest,
        101 => EquipmentSlot::Legs,
        100 => EquipmentSlot::Feet,
        98 => EquipmentSlot::Mainhand,
        99 => EquipmentSlot::Offhand,
        105 => EquipmentSlot::Body,
        106 => EquipmentSlot::Saddle,
        _ => return None,
    })
}

const ENDER_CHEST_SIZE: i32 = 27;

impl ServerLevel {
    pub fn get_slot(&self, id: EntityId, slot: i32) -> Option<LevelSlot> {
        if self.players.contains_key(&id) {
            if slot == 499 {
                return Some(LevelSlot::Carried(id));
            }
            let craft_slot = slot - 500;
            if (0..4).contains(&craft_slot) {
                return Some(LevelSlot::Crafting(id, craft_slot as usize));
            }
            if (0..36).contains(&slot) {
                return Some(LevelSlot::Inventory(id, slot as usize));
            }
            let ender_slot = slot - 200;
            if (0..ENDER_CHEST_SIZE).contains(&ender_slot) {
                return Some(LevelSlot::EnderChest(id, ender_slot as usize));
            }
        }
        if self.is_abstract_chested_horse(id) && slot == super::animal::equine::abstract_horse::CHEST_SLOT_OFFSET {
            return Some(LevelSlot::ChestedHorseChest(id));
        }
        if self.is_abstract_horse(id)
            && let Some(inventory_slot) = self.abstract_horse_get_slot(id, slot)
        {
            return Some(inventory_slot);
        }
        if self.is_abstract_villager(id) {
            let inventory_slot = slot - super::npc::villager::abstract_villager::VILLAGER_SLOT_OFFSET;
            if inventory_slot >= 0 && (inventory_slot as usize) < self.abstract_villager(id).inventory.get_container_size() {
                return Some(LevelSlot::VillagerInventory(id, inventory_slot as usize));
            }
        }
        if let Some(mob) = self.mob(id)
            && let super::mob::MobKind::Pillager(pillager) = &mob.kind
        {
            let inventory_slot = slot - super::monster::illager::pillager::SLOT_OFFSET;
            if inventory_slot >= 0 && (inventory_slot as usize) < pillager.inventory.get_container_size() {
                return Some(LevelSlot::PillagerInventory(id, inventory_slot as usize));
            }
        }
        if self.is_living(id) {
            return get_equipment_slot(slot).map(|slot| LevelSlot::Equipment { entity: id, slot });
        }
        match self.entities.get(&id) {
            Some(LevelEntity::Item(_)) if slot == 0 => Some(LevelSlot::Item(id)),
            Some(LevelEntity::Boat(boat)) if boat.container.is_some() && (0..abstract_chest_boat::CONTAINER_SIZE as i32).contains(&slot) => {
                Some(LevelSlot::Container(ContainerRef::ChestVehicle(id), slot as usize))
            }
            _ => None,
        }
    }

    pub fn get_container_slot(&mut self, container: ContainerRef, slot: i32) -> Option<LevelSlot> {
        (slot >= 0 && (slot as usize) < self.container_size(container)).then_some(LevelSlot::Container(container, slot as usize))
    }

    pub fn get_slot_item(&self, id: EntityId, slot: i32) -> Option<ItemStack> {
        self.get_slot(id, slot).map(|slot| self.entity_slot_get(&slot))
    }

    fn entity_slot_get(&self, slot: &LevelSlot) -> ItemStack {
        match *slot {
            LevelSlot::Equipment { entity, slot } => self.item_by_slot(entity, slot).unwrap_or_else(ItemStack::empty),
            LevelSlot::Carried(player) => self.player(player).carried.clone().unwrap_or_else(ItemStack::empty),
            LevelSlot::Crafting(player, craft_slot) => {
                self.player(player).inventory_menu.as_ref().map_or_else(ItemStack::empty, |menu| menu.containers[0].get_item(craft_slot).clone())
            }
            LevelSlot::Inventory(player, index) => self.player(player).inventory.get_item(index),
            LevelSlot::EnderChest(player, index) => self.player(player).ender_chest_inventory.items.get(index).cloned().unwrap_or_else(ItemStack::empty),
            LevelSlot::VillagerInventory(villager, index) => self.abstract_villager(villager).inventory.get_item(index).clone(),
            LevelSlot::PillagerInventory(pillager, index) => super::monster::illager::pillager::pillager(self.mob(pillager).expect("a pillager")).inventory.get_item(index).clone(),
            LevelSlot::Item(id) => match self.entities.get(&id) {
                Some(LevelEntity::Item(item)) => item.item.clone(),
                _ => ItemStack::empty(),
            },
            LevelSlot::Container(..) => ItemStack::empty(),
            LevelSlot::ChestedHorseChest(id) => self.chested_horse_chest_slot_get(id),
            LevelSlot::Detached(ref item_stack) => item_stack.clone(),
        }
    }

    pub fn level_slot_get(&mut self, slot: &LevelSlot) -> ItemStack {
        match *slot {
            LevelSlot::Container(container, index) => self.get_item(container, index),
            _ => self.entity_slot_get(slot),
        }
    }

    pub fn level_slot_set(&mut self, slot: &LevelSlot, item_stack: ItemStack) -> bool {
        match *slot {
            LevelSlot::Equipment { entity, slot } => {
                let validated = !matches!(slot, EquipmentSlot::Head | EquipmentSlot::Mainhand | EquipmentSlot::Offhand);
                if validated && !(item_stack.is_empty() || self.get_equipment_slot_for_item(entity, &item_stack) == slot) {
                    return false;
                }
                self.living_set_item_slot(entity, slot, item_stack);
            }
            LevelSlot::Carried(player) => self.player_mut(player).carried = (!item_stack.is_empty()).then_some(item_stack),
            LevelSlot::Crafting(player, craft_slot) => {
                let Some(mut inventory_menu) = self.player_mut(player).inventory_menu.take() else { return true };
                self.menu_container_set_item(&mut inventory_menu, player, 0, craft_slot, item_stack);
                self.menu_slots_changed(&mut inventory_menu, player, 0);
                self.restore_container_menu(player, inventory_menu);
            }
            LevelSlot::Inventory(player, index) => self.player_mut(player).inventory.slots[inventory_slot(index)] = (!item_stack.is_empty()).then_some(item_stack),
            LevelSlot::EnderChest(player, index) => self.set_item(ContainerRef::PlayerEnderChest(player), index, item_stack),
            LevelSlot::VillagerInventory(villager, index) => self.abstract_villager_mut(villager).inventory.set_item(index, item_stack),
            LevelSlot::PillagerInventory(pillager, index) => super::monster::illager::pillager::pillager_mut(self.mob_mut(pillager).expect("a pillager")).inventory.set_item(index, item_stack),
            LevelSlot::Item(id) => {
                if let Some(LevelEntity::Item(item)) = self.entities.get_mut(&id) {
                    item.set_item(item_stack);
                }
            }
            LevelSlot::Container(container, index) => self.set_item(container, index, item_stack),
            LevelSlot::ChestedHorseChest(id) => return self.chested_horse_chest_slot_set(id, &item_stack),
            LevelSlot::Detached(_) => {}
        }
        true
    }

    pub fn level_slot_modify(&mut self, slot: &LevelSlot, modifier: &mut dyn FnMut(&mut ServerLevel, &mut ItemStack)) {
        let mut item_stack = self.level_slot_get(slot);
        let was_empty = item_stack.is_empty();
        modifier(self, &mut item_stack);
        if was_empty {
            return;
        }
        match *slot {
            LevelSlot::Equipment { entity, slot } => self.set_item_slot(entity, slot, item_stack),
            LevelSlot::Carried(player) => self.player_mut(player).carried = Some(item_stack),
            LevelSlot::Crafting(player, craft_slot) => {
                if let Some(menu) = self.player_mut(player).inventory_menu.as_mut() {
                    *menu.containers[0].get_item_mut(craft_slot) = item_stack;
                }
            }
            LevelSlot::Inventory(player, index) => self.player_mut(player).inventory.slots[inventory_slot(index)] = Some(item_stack),
            LevelSlot::EnderChest(player, index) => self.put_item(ContainerRef::PlayerEnderChest(player), index, item_stack),
            LevelSlot::VillagerInventory(villager, index) => self.abstract_villager_mut(villager).inventory.items[index] = item_stack,
            LevelSlot::PillagerInventory(pillager, index) => super::monster::illager::pillager::pillager_mut(self.mob_mut(pillager).expect("a pillager")).inventory.items[index] = item_stack,
            LevelSlot::Item(id) => {
                if let Some(LevelEntity::Item(item)) = self.entities.get_mut(&id) {
                    item.item = item_stack;
                }
            }
            LevelSlot::Container(container, index) => self.put_item(container, index, item_stack),
            LevelSlot::ChestedHorseChest(_) => {}
            LevelSlot::Detached(_) => {}
        }
    }
}
