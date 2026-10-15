use super::abstract_container_menu::AbstractContainerMenu;
use super::merchant_container::CONTAINER;
use super::slot::SlotKind;
use crate::server::level::ServerLevel;
use crate::stats::stats;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

impl ServerLevel {
    pub fn merchant_result_slot_may_pickup(&self, menu: &AbstractContainerMenu) -> bool {
        let state = menu.merchant_state();
        let Some(offer) = state.container.active_offer else { return false };
        let Some(offers) = self.mob(state.trader).and_then(|mob| mob.abstract_villager()).and_then(|villager| villager.offers.as_ref()) else { return false };
        let offer = &offers.0[offer];
        let buy_a = menu.containers[CONTAINER].get_item(0);
        let buy_b = menu.containers[CONTAINER].get_item(1);
        offer.satisfied_by(buy_a, buy_b) || offer.satisfied_by(buy_b, buy_a)
    }

    pub fn merchant_result_slot_check_take_achievements(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, carried: &mut ItemStack) {
        if let SlotKind::MerchantResult { remove_count } = &mut menu.slots[index].kind {
            let count = *remove_count;
            *remove_count = 0;
            carried.on_crafted_by(self, player, count);
        }
    }

    pub fn merchant_result_slot_on_take(&mut self, menu: &mut AbstractContainerMenu, player: EntityId, index: usize, carried: &mut ItemStack) {
        self.merchant_result_slot_check_take_achievements(menu, player, index, carried);
        let state = menu.merchant_state();
        let trader = state.trader;
        let Some(offer_index) = state.container.active_offer else { return };
        let offer = self.get_offers(trader).0[offer_index].clone();
        let mut buy_a = menu.containers[CONTAINER].get_item(0).clone();
        let mut buy_b = menu.containers[CONTAINER].get_item(1).clone();
        if offer.take(&mut buy_a, &mut buy_b) || offer.take(&mut buy_b, &mut buy_a) {
            self.notify_trade(trader, offer_index);
            self.award_stat(player, stats::TRADED_WITH_VILLAGER, 1);
            self.merchant_container_set_item(menu, 0, buy_a);
            self.merchant_container_set_item(menu, 1, buy_b);
        }
    }
}
