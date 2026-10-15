use wafer_protocol::nbt::Nbt;

use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::world::entity::ageable_mob::{AgeableMob, AgeableMobGroupData};
use crate::world::entity::data::DataValue;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::npc::inventory_carrier;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityId, sound_event};
use crate::world::item::ItemStack;
use crate::world::item::trading::merchant_offers::MerchantOffers;
use crate::world::item::trading::trade_set::TradeSet;
use crate::world::item::trading::villager_trade::VillagerTrade;
use crate::world::level::pathfinder::path_type::PathType;
use crate::world::level::storage::loot::loot_context::{LootContext, LootContextBuilder};
use crate::world::level::storage::loot::loot_params::LootParams;
use crate::world::simple_container::{SimpleContainer, SimpleContainerKind};

pub const DATA_UNHAPPY_COUNTER: usize = 18;
pub const VILLAGER_SLOT_OFFSET: i32 = 300;
const VILLAGER_INVENTORY_SIZE: usize = 8;

pub struct AbstractVillager {
    pub ageable: AgeableMob,
    pub trading_player: Option<EntityId>,
    pub offers: Option<MerchantOffers>,
    pub inventory: SimpleContainer,
}

impl Default for AbstractVillager {
    fn default() -> AbstractVillager {
        AbstractVillager {
            ageable: AgeableMob::default(),
            trading_player: None,
            offers: None,
            inventory: SimpleContainer::new(SimpleContainerKind::Plain, VILLAGER_INVENTORY_SIZE),
        }
    }
}

impl AbstractVillager {
    pub fn init(mob: &mut Mob) {
        mob.set_pathfinding_malus(PathType::FireInNeighbor, 16.0);
        mob.set_pathfinding_malus(PathType::Fire, -1.0);
    }

    pub fn add_additional_save_data(mob: &Mob, out: &mut Vec<(String, Nbt)>) {
        let abstract_villager = mob.abstract_villager().expect("an abstract villager");
        if let Some(offers) = &abstract_villager.offers {
            out.push(("Offers".into(), offers.to_nbt()));
        }
        inventory_carrier::write_inventory_to_tag(&abstract_villager.inventory, out);
    }

    pub fn read_additional_save_data(mob: &mut Mob, nbt: &Nbt) {
        let abstract_villager = mob.abstract_villager_mut().expect("an abstract villager");
        abstract_villager.offers = nbt.get("Offers").and_then(|offers| MerchantOffers::decode(offers).ok());
        inventory_carrier::read_inventory_from_tag(&mut abstract_villager.inventory, nbt);
    }
}

pub fn finalize_spawn(group_data: Option<SpawnGroupData>) -> Option<SpawnGroupData> {
    Some(group_data.unwrap_or(SpawnGroupData::AgeableMob(AgeableMobGroupData::new(false))))
}

pub fn get_unhappy_counter(mob: &Mob) -> i32 {
    match mob.entity.data.get(DATA_UNHAPPY_COUNTER) {
        DataValue::Int(value) => *value,
        _ => 0,
    }
}

pub fn set_unhappy_counter(mob: &mut Mob, value: i32) {
    mob.entity.data.set(DATA_UNHAPPY_COUNTER, DataValue::Int(value));
}

impl Mob {
    pub fn abstract_villager(&self) -> Option<&AbstractVillager> {
        match &self.kind {
            MobKind::Villager(villager) => Some(&villager.abstract_villager),
            _ => None,
        }
    }

    pub fn abstract_villager_mut(&mut self) -> Option<&mut AbstractVillager> {
        match &mut self.kind {
            MobKind::Villager(villager) => Some(&mut villager.abstract_villager),
            _ => None,
        }
    }
}

impl ServerLevel {
    pub fn abstract_villager(&self, id: EntityId) -> &AbstractVillager {
        self.mob(id).and_then(Mob::abstract_villager).expect("an abstract villager")
    }

    pub fn abstract_villager_mut(&mut self, id: EntityId) -> &mut AbstractVillager {
        self.mob_mut(id).and_then(Mob::abstract_villager_mut).expect("an abstract villager")
    }

    pub fn is_abstract_villager(&self, id: EntityId) -> bool {
        self.mob(id).and_then(Mob::abstract_villager).is_some()
    }

    pub fn set_trading_player(&mut self, id: EntityId, player: Option<EntityId>) {
        match self.mob(id).map(|mob| &mob.kind) {
            Some(MobKind::Villager(_)) => self.villager_set_trading_player(id, player),
            _ => self.abstract_villager_set_trading_player(id, player),
        }
    }

    pub fn abstract_villager_set_trading_player(&mut self, id: EntityId, player: Option<EntityId>) {
        self.abstract_villager_mut(id).trading_player = player;
    }

    pub fn get_trading_player(&self, id: EntityId) -> Option<EntityId> {
        self.abstract_villager(id).trading_player
    }

    pub fn is_trading(&self, id: EntityId) -> bool {
        self.abstract_villager(id).trading_player.is_some()
    }

    pub fn get_offers(&mut self, id: EntityId) -> &MerchantOffers {
        if self.abstract_villager(id).offers.is_none() {
            self.abstract_villager_mut(id).offers = Some(MerchantOffers::default());
            self.update_trades(id);
        }
        self.abstract_villager(id).offers.as_ref().expect("offers")
    }

    pub fn update_trades(&mut self, id: EntityId) {
        if let Some(MobKind::Villager(_)) = self.mob(id).map(|mob| &mob.kind) {
            self.villager_update_trades(id);
        }
    }

    pub fn notify_trade(&mut self, id: EntityId, offer: usize) {
        let interval = self.mob(id).expect("an abstract villager").ambient_sound_interval();
        let abstract_villager = self.abstract_villager_mut(id);
        let offers = abstract_villager.offers.as_mut().expect("offers");
        offers.0[offer].increase_uses();
        let offer_copy = offers.0[offer].clone();
        self.mob_mut(id).expect("an abstract villager").ambient_sound_time = -interval;
        self.reward_trade_xp(id, &offer_copy);
        if let Some(trading_player) = self.abstract_villager(id).trading_player.filter(|&player| self.is_player(player)) {
            criteria_triggers::TRADE.trigger(self, trading_player, id, offer_copy.get_result());
        }
    }

    fn reward_trade_xp(&mut self, id: EntityId, offer: &crate::world::item::trading::merchant_offer::MerchantOffer) {
        if let Some(MobKind::Villager(_)) = self.mob(id).map(|mob| &mob.kind) {
            self.villager_reward_trade_xp(id, offer);
        }
    }

    pub fn show_progress_bar(&self, _id: EntityId) -> bool {
        true
    }

    pub fn notify_trade_updated(&mut self, id: EntityId, item_stack: &ItemStack) {
        let mob = self.mob(id).expect("an abstract villager");
        let interval = mob.ambient_sound_interval();
        if mob.ambient_sound_time > -interval + 20 {
            self.mob_mut(id).expect("an abstract villager").ambient_sound_time = -interval;
            let sound = get_trade_updated_sound(!item_stack.is_empty());
            self.make_sound(id, sound_event(sound));
        }
    }

    pub fn get_notify_trade_sound(&self, _id: EntityId) -> &'static str {
        "minecraft:entity.villager.yes"
    }

    pub fn play_celebrate_sound(&mut self, id: EntityId) {
        self.make_sound(id, sound_event("minecraft:entity.villager.celebrate"));
    }

    pub fn stop_trading(&mut self, id: EntityId) {
        self.set_trading_player(id, None);
    }

    pub fn abstract_villager_die(&mut self, id: EntityId, source: crate::world::damagesource::DamageSource) {
        self.living_entity_die(id, source);
        self.stop_trading(id);
    }

    pub fn merchant_still_valid(&self, id: EntityId, player: EntityId) -> bool {
        self.mob(id).and_then(Mob::abstract_villager).is_some_and(|villager| villager.trading_player == Some(player))
            && self.is_alive(id)
            && self.is_within_entity_interaction_range(player, id, 4.0)
    }

    pub fn add_offers_from_trade_set(&mut self, id: EntityId, resource_key: &str) {
        let Some(trade_set) = TradeSet::get_optional(resource_key) else { return };
        let origin = self.entity(id).position();
        let mut params = LootParams::new(self);
        params.origin = Some(origin);
        params.this_entity = Some(id);
        params.additional_cost_component_allowed = true;
        let mut loot_context = LootContextBuilder::new(params).create(trade_set.random_sequence.as_deref());
        let number_of_offers = trade_set.calculate_number_of_trades(&mut loot_context);
        let mut found = Vec::new();
        if trade_set.allow_duplicates {
            add_offers_from_item_listings(&mut loot_context, &mut found, trade_set.trades.contents(), number_of_offers);
        } else {
            add_offers_from_item_listings_without_duplicates(&mut loot_context, &mut found, trade_set.trades.contents(), number_of_offers);
        }
        drop(loot_context);
        self.abstract_villager_mut(id).offers.get_or_insert_with(MerchantOffers::default).0.extend(found);
    }
}

pub fn add_offers_from_item_listings(loot_context: &mut LootContext, merchant_offers: &mut Vec<crate::world::item::trading::merchant_offer::MerchantOffer>, potential_offers: &[i32], number_of_offers: i32) {
    let mut potential_offers_list = potential_offers.to_vec();
    let mut offers_found = 0;
    while offers_found < number_of_offers && !potential_offers_list.is_empty() {
        let roll = wafer_core::random::RandomSource::next_int_bounded(&mut loot_context.random(), potential_offers_list.len() as i32) as usize;
        let villager_trade = VillagerTrade::by_id(potential_offers_list[roll]);
        match villager_trade.get_offer(loot_context) {
            None => {
                potential_offers_list.remove(roll);
            }
            Some(offer) => {
                merchant_offers.push(offer);
                offers_found += 1;
            }
        }
    }
}

fn add_offers_from_item_listings_without_duplicates(
    loot_context: &mut LootContext,
    merchant_offers: &mut Vec<crate::world::item::trading::merchant_offer::MerchantOffer>,
    potential_offers: &[i32],
    number_of_offers: i32,
) {
    let mut leftover_offers = potential_offers.to_vec();
    let mut offers_found = 0;
    while offers_found < number_of_offers && !leftover_offers.is_empty() {
        let roll = wafer_core::random::RandomSource::next_int_bounded(&mut loot_context.random(), leftover_offers.len() as i32) as usize;
        let villager_trade = VillagerTrade::by_id(leftover_offers.remove(roll));
        if let Some(offer) = villager_trade.get_offer(loot_context) {
            merchant_offers.push(offer);
            offers_found += 1;
        }
    }
}

fn get_trade_updated_sound(valid_trade: bool) -> &'static str {
    if valid_trade { "minecraft:entity.villager.yes" } else { "minecraft:entity.villager.no" }
}
