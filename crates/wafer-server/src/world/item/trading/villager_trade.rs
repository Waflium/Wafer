use std::sync::OnceLock;

use wafer_protocol::nbt::Nbt;

use super::merchant_offer::MerchantOffer;
use super::trade_cost::TradeCost;
use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::codec::registry_id;
use crate::world::item::component::values::{ItemEnchantments, ItemStackTemplate};
use crate::world::item::component as dc;
use crate::world::level::storage::loot::functions::loot_item_function::LootItemFunction;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::predicates::loot_item_condition::LootItemCondition;
use crate::world::level::storage::loot::providers::number::floats::{self, ContextFloatProvider};
use crate::world::level::storage::loot::providers::number::ints::{self, ContextIntProvider};
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;
use crate::world::level::storage::loot::parameters::loot_context_param_sets;

pub struct VillagerTrade {
    wants: TradeCost,
    additional_wants: Option<TradeCost>,
    gives: ItemStackTemplate,
    merchant_predicate: Option<Holder<LootItemCondition>>,
    given_item_modifier: Option<Holder<LootItemFunction>>,
    max_uses: Holder<ContextIntProvider>,
    xp: Holder<ContextIntProvider>,
    reputation_discount: Holder<ContextFloatProvider>,
    double_trade_price_enchantments: Option<HolderSet>,
}

pub const REGISTRY: &str = "minecraft:villager_trade";

impl VillagerTrade {
    pub fn decode(tag: &Nbt) -> DataResult<VillagerTrade> {
        VillagerTrade::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<VillagerTrade> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = codec::Record::default();
        let wants = record.required(tag, "wants", TradeCost::decode);
        let additional_wants = record.optional(tag, "additional_wants", TradeCost::decode);
        let gives = record.required(tag, "gives", ItemStackTemplate::decode_partial);
        let max_uses = record.with_default(tag, "max_uses", ints::exactly(4), ContextIntProvider::decode_holder);
        let xp = record.with_default(tag, "xp", ints::exactly(1), ContextIntProvider::decode_holder);
        let reputation_discount = record.with_default(tag, "reputation_discount", floats::exactly(0.0), ContextFloatProvider::decode_holder);
        let merchant_predicate = record.optional(tag, "merchant_predicate", LootItemCondition::decode_holder_partial);
        let given_item_modifier = record.optional(tag, "given_item_modifier", LootItemFunction::decode_holder);
        let double_trade_price_enchantments = record.optional(tag, "double_trade_price_enchantments", |t| HolderSet::decode_partial("minecraft:enchantment", t));
        record
            .build(|| {
                Some(VillagerTrade {
                    wants: wants?,
                    additional_wants,
                    gives: gives?,
                    max_uses,
                    xp,
                    reputation_discount,
                    merchant_predicate,
                    given_item_modifier,
                    double_trade_price_enchantments,
                })
            })
            .validate(|trade| validatable::validator_for_context(&loot_context_param_sets::VILLAGER_TRADE, trade))
    }

    pub fn by_id(id: i32) -> &'static VillagerTrade {
        static TRADES: OnceLock<Vec<VillagerTrade>> = OnceLock::new();
        &TRADES.get_or_init(|| {
            crate::core::data_registries::json_entries("minecraft:villager_trade")
                .iter()
                .map(|&(name, json)| {
                    let tag = crate::world::item::component::json_to_nbt(&serde_json::from_str(json).expect("vanilla JSON"));
                    VillagerTrade::decode(&tag).unwrap_or_else(|e| panic!("{name}: {e}"))
                })
                .collect()
        })[id as usize]
    }

    pub fn get_offer(&self, loot_context: &mut LootContext) -> Option<MerchantOffer> {
        if let Some(merchant_predicate) = &self.merchant_predicate
            && !merchant_predicate.value().test(loot_context)
        {
            return None;
        }
        let mut result = self.gives.create().unwrap_or_else(crate::world::item::ItemStack::empty);
        let mut additional_cost = 0;
        if let Some(given_item_modifier) = &self.given_item_modifier {
            result = given_item_modifier.value().apply(result, loot_context);
            if result.is_empty() {
                return None;
            }
        }
        if let Some(additional_trade_cost) = result.remove(dc::ADDITIONAL_TRADE_COST).as_ref().and_then(Nbt::as_i64) {
            additional_cost += additional_trade_cost as i32;
        }
        if let Some(enchantments) = &self.double_trade_price_enchantments
            && let Some(item_enchantments) = result.get(dc::STORED_ENCHANTMENTS).map(ItemEnchantments::from_nbt)
        {
            if item_enchantments.enchantments.iter().any(|(key, _)| registry_id("minecraft:enchantment", key).is_ok_and(|id| enchantments.contains(id))) {
                additional_cost *= 2;
            }
        }
        let item_cost = self.wants.to_item_cost(loot_context, additional_cost);
        if item_cost.count < 1 {
            return None;
        }
        let additional_item_cost = self.additional_wants.as_ref().map(|trade_cost| trade_cost.to_item_cost(loot_context, 0));
        if additional_item_cost.as_ref().is_some_and(|cost| cost.count < 1) {
            return None;
        }
        Some(MerchantOffer::new(
            item_cost,
            additional_item_cost,
            result,
            self.max_uses.value().get_int(loot_context).max(1),
            self.xp.value().get_int(loot_context).max(0),
            self.reputation_discount.value().get_float(loot_context).max(0.0),
        ))
    }
}

impl Validatable for VillagerTrade {
    fn validate(&self, context: &ValidationContext) {
        validatable::validate(context, "wants", &self.wants);
        validatable::validate_optional(context, "additional_wants", self.additional_wants.as_ref());
        validatable::validate_holder(context, "max_uses", &self.max_uses);
        validatable::validate_holder(context, "reputation_discount", &self.reputation_discount);
        validatable::validate_holder(context, "xp", &self.xp);
        validatable::validate_optional_holder(context, "merchant_predicate", self.merchant_predicate.as_ref());
        validatable::validate_optional_holder(context, "given_item_modifier", self.given_item_modifier.as_ref());
    }
}
