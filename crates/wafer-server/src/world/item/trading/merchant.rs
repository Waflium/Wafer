use wafer_data::packets::play::clientbound;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::inventory::menu_provider::MenuProvider;

impl ServerLevel {
    pub fn can_restock(&self, id: EntityId) -> bool {
        self.is_villager(id)
    }

    pub fn open_trading_screen(&mut self, merchant: EntityId, player: EntityId, level: i32) {
        if self.open_menu(player, MenuProvider::Merchant(merchant)) {
            let container_id = self.container_menu_id(player);
            if !self.get_offers(merchant).is_empty() {
                let xp = self.get_villager_xp(merchant);
                let show_progress_bar = self.show_progress_bar(merchant);
                let can_restock = self.can_restock(merchant);
                self.send_merchant_offers(player, merchant, container_id, level, xp, show_progress_bar, can_restock);
            }
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn send_merchant_offers(&mut self, player: EntityId, merchant: EntityId, container_id: i32, merchant_level: i32, merchant_xp: i32, show_progress_bar: bool, can_restock: bool) {
        let offers = self.get_offers(merchant).clone();
        self.player(player).connection.send(clientbound::MERCHANT_OFFERS, |w| {
            w.varint(container_id);
            offers.write(w);
            w.varint(merchant_level);
            w.varint(merchant_xp);
            w.bool(show_progress_bar);
            w.bool(can_restock);
        });
    }
}
