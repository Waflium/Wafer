use super::abstract_container_menu::{AbstractContainerMenu, MenuKind};
use super::merchant_container::{CONTAINER, MerchantContainer};
use crate::server::level::ServerLevel;
use crate::util::Prediction;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::trading::item_cost::ItemCost;

pub const PAYMENT1_SLOT: usize = 0;
pub const PAYMENT2_SLOT: usize = 1;
pub const RESULT_SLOT: usize = 2;
pub const INV_SLOT_START: usize = 3;
pub const INV_SLOT_END: usize = 30;
pub const USE_ROW_SLOT_START: usize = 30;
pub const USE_ROW_SLOT_END: usize = 39;

pub struct MerchantMenu {
    pub trader: EntityId,
    pub container: MerchantContainer,
    merchant_level: i32,
    show_progress_bar: bool,
    can_restock: bool,
}

impl AbstractContainerMenu {
    pub fn merchant(container_id: i32, trader: EntityId) -> AbstractContainerMenu {
        let mut menu = AbstractContainerMenu::new(container_id, MenuKind::Merchant, None);
        menu.merchant = Some(MerchantMenu { trader, container: MerchantContainer::default(), merchant_level: 0, show_progress_bar: false, can_restock: false });
        menu
    }

    pub fn merchant_state(&self) -> &MerchantMenu {
        self.merchant.as_ref().expect("a merchant menu")
    }

    pub fn merchant_state_mut(&mut self) -> &mut MerchantMenu {
        self.merchant.as_mut().expect("a merchant menu")
    }
}

impl MerchantMenu {
    pub fn set_show_progress_bar(&mut self, show: bool) {
        self.show_progress_bar = show;
    }

    pub fn set_merchant_level(&mut self, level: i32) {
        self.merchant_level = level;
    }

    pub fn set_can_restock(&mut self, can_restock: bool) {
        self.can_restock = can_restock;
    }
}

impl ServerLevel {
    pub fn merchant_menu_update_sell_item(&mut self, player: EntityId) {
        let Some(mut menu) = self.player_mut(player).container_menu.take() else { return };
        if menu.kind == MenuKind::Merchant {
            self.merchant_container_update_sell_item(&mut menu);
        }
        self.restore_container_menu(player, menu);
    }

    pub fn merchant_menu_set_selection_hint(&mut self, menu: &mut AbstractContainerMenu, hint: i32) {
        self.merchant_container_set_selection_hint(menu, hint);
    }

    pub fn merchant_menu_still_valid(&self, menu: &AbstractContainerMenu, player: EntityId) -> bool {
        self.merchant_still_valid(menu.merchant_state().trader, player)
    }

    pub fn merchant_menu_quick_move_stack(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, slot_index: usize, stack: &mut ItemStack, clicked: &ItemStack) -> bool {
        if slot_index == RESULT_SLOT {
            if !self.move_item_stack_to(menu, player, stack, INV_SLOT_START, USE_ROW_SLOT_END, true) {
                return false;
            }
            self.slot_on_quick_craft(menu, player, slot_index, stack, clicked);
            true
        } else if slot_index != PAYMENT1_SLOT && slot_index != PAYMENT2_SLOT {
            if (INV_SLOT_START..INV_SLOT_END).contains(&slot_index) {
                self.move_item_stack_to(menu, player, stack, USE_ROW_SLOT_START, USE_ROW_SLOT_END, false)
            } else {
                !((USE_ROW_SLOT_START..USE_ROW_SLOT_END).contains(&slot_index) && !self.move_item_stack_to(menu, player, stack, INV_SLOT_START, USE_ROW_SLOT_START, false))
            }
        } else {
            self.move_item_stack_to(menu, player, stack, INV_SLOT_START, USE_ROW_SLOT_END, false)
        }
    }

    pub fn merchant_menu_removed(&mut self, menu: &mut AbstractContainerMenu, player: EntityId) {
        let trader = menu.merchant_state().trader;
        if self.entity_ref(trader).is_some() {
            self.set_trading_player(trader, None);
        }
        if !self.is_alive(player) || self.player(player).disconnected {
            for slot in [PAYMENT1_SLOT, PAYMENT2_SLOT] {
                let item_stack = self.menu_container_remove_item_no_update(menu, CONTAINER, slot);
                if !item_stack.is_empty() {
                    self.player_drop(player, item_stack, false, Prediction::ServerOnly);
                }
            }
        } else {
            for slot in [PAYMENT1_SLOT, PAYMENT2_SLOT] {
                let mut item_stack = self.menu_container_remove_item_no_update(menu, CONTAINER, slot);
                self.place_item_back_in_inventory(player, &mut item_stack, true);
            }
        }
    }

    pub fn merchant_menu_try_move_items(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, new_trade_index: i32) {
        let trader = menu.merchant_state().trader;
        if new_trade_index < 0 || self.get_offers(trader).len() <= new_trade_index as usize {
            return;
        }
        for slot in [PAYMENT1_SLOT, PAYMENT2_SLOT] {
            let mut old_cost = menu.containers[CONTAINER].get_item(slot).clone();
            if !old_cost.is_empty() {
                if !self.move_item_stack_to(menu, player, &mut old_cost, INV_SLOT_START, USE_ROW_SLOT_END, true) {
                    return;
                }
                self.merchant_container_set_item(menu, slot, old_cost);
            }
        }
        if menu.containers[CONTAINER].get_item(PAYMENT1_SLOT).is_empty() && menu.containers[CONTAINER].get_item(PAYMENT2_SLOT).is_empty() {
            let offer = self.get_offers(trader).0[new_trade_index as usize].clone();
            self.move_from_inventory_to_payment_slot(menu, player, PAYMENT1_SLOT, offer.get_item_cost_a());
            if let Some(cost_b) = offer.get_item_cost_b() {
                self.move_from_inventory_to_payment_slot(menu, player, PAYMENT2_SLOT, cost_b);
            }
        }
    }

    fn move_from_inventory_to_payment_slot(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, payment_slot: usize, cost: &ItemCost) {
        for i in INV_SLOT_START..USE_ROW_SLOT_END {
            let mut inventory_item = self.slot_get(menu, player, i);
            if !inventory_item.is_empty() && cost.test(&inventory_item) {
                let current_payment_item = menu.containers[CONTAINER].get_item(payment_slot).clone();
                if current_payment_item.is_empty() || ItemStack::is_same_item_same_components(&inventory_item, &current_payment_item) {
                    let max_stack_size = inventory_item.max_stack_size();
                    let move_count = (max_stack_size - current_payment_item.count).min(inventory_item.count);
                    let new_payment_item = inventory_item.copy_with_count(current_payment_item.count + move_count);
                    inventory_item.shrink(move_count);
                    self.slot_put(menu, player, i, inventory_item);
                    let reached_max = new_payment_item.count >= max_stack_size;
                    self.merchant_container_set_item(menu, payment_slot, new_payment_item);
                    if reached_max {
                        break;
                    }
                }
            }
        }
    }

    pub fn handle_select_trade(&mut self, player: EntityId, selection: i32) {
        let Some(mut menu) = self.player_mut(player).container_menu.take() else { return };
        if menu.kind == MenuKind::Merchant && self.merchant_menu_still_valid(&menu, player) {
            self.merchant_menu_set_selection_hint(&mut menu, selection);
            self.merchant_menu_try_move_items(&mut menu, player, selection);
        }
        self.restore_container_menu(player, menu);
    }
}
