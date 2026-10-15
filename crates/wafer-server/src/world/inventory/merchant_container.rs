use super::abstract_container_menu::AbstractContainerMenu;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub const CONTAINER: usize = 0;

#[derive(Default)]
pub struct MerchantContainer {
    pub active_offer: Option<usize>,
    pub selection_hint: i32,
    pub future_xp: i32,
}

pub fn is_payment_slot(slot: usize) -> bool {
    slot == 0 || slot == 1
}

impl ServerLevel {
    pub fn merchant_container_set_item(&mut self, menu: &mut AbstractContainerMenu, slot: usize, mut item_stack: ItemStack) {
        let max = menu.containers[CONTAINER].get_max_stack_size().min(item_stack.max_stack_size());
        item_stack.limit_size(max);
        *menu.containers[CONTAINER].get_item_mut(slot) = item_stack;
        if is_payment_slot(slot) {
            self.merchant_container_update_sell_item(menu);
        }
    }

    pub fn merchant_container_remove_item(&mut self, menu: &mut AbstractContainerMenu, slot: usize, count: i32) -> ItemStack {
        let item_stack = menu.containers[CONTAINER].get_item_mut(slot);
        if slot == 2 && !item_stack.is_empty() {
            let full = item_stack.count;
            return item_stack.split(full);
        }
        let result = if !item_stack.is_empty() && count > 0 { item_stack.split(count) } else { ItemStack::empty() };
        if !result.is_empty() && is_payment_slot(slot) {
            self.merchant_container_update_sell_item(menu);
        }
        result
    }

    pub fn merchant_container_update_sell_item(&mut self, menu: &mut AbstractContainerMenu) {
        let trader = menu.merchant_state().trader;
        menu.merchant_state_mut().container.active_offer = None;
        let (buy_a, buy_b) = if menu.containers[CONTAINER].get_item(0).is_empty() {
            (menu.containers[CONTAINER].get_item(1).clone(), ItemStack::empty())
        } else {
            (menu.containers[CONTAINER].get_item(0).clone(), menu.containers[CONTAINER].get_item(1).clone())
        };
        if buy_a.is_empty() {
            self.merchant_container_set_item(menu, 2, ItemStack::empty());
            menu.merchant_state_mut().container.future_xp = 0;
            return;
        }
        let selection_hint = menu.merchant_state().container.selection_hint;
        let offers = self.get_offers(trader);
        if !offers.is_empty() {
            let mut offer = offers.get_recipe_for(&buy_a, &buy_b, selection_hint);
            if offer.is_none_or(|offer| offers.0[offer].is_out_of_stock()) {
                menu.merchant_state_mut().container.active_offer = offer;
                offer = offers.get_recipe_for(&buy_b, &buy_a, selection_hint);
            }
            match offer.filter(|&offer| !offers.0[offer].is_out_of_stock()) {
                Some(offer) => {
                    let result = offers.0[offer].assemble();
                    let xp = offers.0[offer].get_xp();
                    menu.merchant_state_mut().container.active_offer = Some(offer);
                    self.merchant_container_set_item(menu, 2, result);
                    menu.merchant_state_mut().container.future_xp = xp;
                }
                None => {
                    self.merchant_container_set_item(menu, 2, ItemStack::empty());
                    menu.merchant_state_mut().container.future_xp = 0;
                }
            }
        }
        let result = menu.containers[CONTAINER].get_item(2).clone();
        self.notify_trade_updated(trader, &result);
    }

    pub fn merchant_container_set_selection_hint(&mut self, menu: &mut AbstractContainerMenu, selection_hint: i32) {
        menu.merchant_state_mut().container.selection_hint = selection_hint;
        self.merchant_container_update_sell_item(menu);
    }
}
