use super::abstract_container_menu::AbstractContainerMenu;
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

pub const SLOT_SADDLE: usize = 0;
pub const SLOT_BODY_ARMOR: usize = 1;
pub const SLOT_INVENTORY_START: usize = 2;
pub const INVENTORY_ROWS: usize = 3;

pub fn get_inventory_size(inventory_columns: usize) -> usize {
    inventory_columns * INVENTORY_ROWS
}

impl ServerLevel {
    pub fn abstract_mount_inventory_menu_still_valid(&mut self, menu: &AbstractContainerMenu, player: EntityId, mount: EntityId, inventory_version: u32) -> bool {
        let mount_container = menu.container.unwrap_or(ContainerRef::HorseInventory(mount));
        !self.has_inventory_changed(mount, inventory_version)
            && self.container_still_valid(mount_container, player)
            && self.entity_ref(mount).is_some()
            && self.is_alive(mount)
            && self.is_within_entity_interaction_range(player, mount, 4.0)
    }

    pub fn abstract_mount_inventory_menu_quick_move_stack(
        &mut self,
        menu: &mut AbstractContainerMenu,
        player: EntityId,
        slot_index: usize,
        mut stack: ItemStack,
        clicked: ItemStack,
        mount_container_size: usize,
    ) -> ItemStack {
        let player_container_start = SLOT_INVENTORY_START + mount_container_size;
        let total = menu.slots.len();
        if slot_index < player_container_start {
            if !self.move_item_stack_to(menu, player, &mut stack, player_container_start, total, true) {
                return ItemStack::empty();
            }
        } else if self.slot_may_place(menu, player, SLOT_BODY_ARMOR, &stack) && !self.slot_has_item(menu, player, SLOT_BODY_ARMOR) {
            if !self.move_item_stack_to(menu, player, &mut stack, SLOT_BODY_ARMOR, SLOT_BODY_ARMOR + 1, false) {
                return ItemStack::empty();
            }
        } else if self.slot_may_place(menu, player, SLOT_SADDLE, &stack) && !self.slot_has_item(menu, player, SLOT_SADDLE) {
            if !self.move_item_stack_to(menu, player, &mut stack, SLOT_SADDLE, SLOT_SADDLE + 1, false) {
                return ItemStack::empty();
            }
        } else if mount_container_size == 0 || !self.move_item_stack_to(menu, player, &mut stack, SLOT_INVENTORY_START, player_container_start, false) {
            let player_container_end = player_container_start + 27;
            let player_hot_bar_start = player_container_end;
            let player_hot_bar_end = player_hot_bar_start + 9;
            let moved = if (player_hot_bar_start..player_hot_bar_end).contains(&slot_index) {
                self.move_item_stack_to(menu, player, &mut stack, player_container_start, player_container_end, false)
            } else if (player_container_start..player_container_end).contains(&slot_index) {
                self.move_item_stack_to(menu, player, &mut stack, player_hot_bar_start, player_hot_bar_end, false)
            } else {
                self.move_item_stack_to(menu, player, &mut stack, player_hot_bar_start, player_container_end, false)
            };
            if moved {
                self.slot_put(menu, player, slot_index, stack);
            }
            return ItemStack::empty();
        }
        self.slot_put(menu, player, slot_index, stack.clone());
        if stack.is_empty() {
            self.slot_set_by_player(menu, player, slot_index, ItemStack::empty());
        } else {
            self.slot_set_changed(menu, player, slot_index);
        }
        clicked
    }
}
