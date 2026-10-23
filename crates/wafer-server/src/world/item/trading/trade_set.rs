use std::collections::HashMap;
use std::sync::OnceLock;

use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::ContextIntProvider;

pub struct TradeSet {
    pub trades: HolderSet,
    pub amount: Holder<ContextIntProvider>,
    pub allow_duplicates: bool,
    pub random_sequence: Option<String>,
}

impl TradeSet {
    pub fn decode(tag: &Nbt) -> DataResult<TradeSet> {
        codec::map(tag)?;
        Ok(TradeSet {
            trades: codec::decode_field(tag, "trades", |t| HolderSet::decode(super::villager_trade::REGISTRY, t))?,
            amount: codec::decode_field(tag, "amount", ContextIntProvider::decode_holder)?,
            allow_duplicates: codec::decode_or(tag, "allow_duplicates", false, codec::bool_value)?,
            random_sequence: codec::decode_optional(tag, "random_sequence", codec::identifier)?,
        })
    }

    pub fn get_optional(key: &str) -> Option<&'static TradeSet> {
        static TRADE_SETS: OnceLock<HashMap<&'static str, TradeSet>> = OnceLock::new();
        TRADE_SETS
            .get_or_init(|| {
                crate::core::data_registries::json_entries("minecraft:trade_set")
                    .iter()
                    .map(|&(name, json)| {
                        let tag = crate::world::item::component::json_to_nbt(&serde_json::from_str(json).expect("vanilla JSON"));
                        (name, TradeSet::decode(&tag).unwrap_or_else(|e| panic!("{name}: {e}")))
                    })
                    .collect()
            })
            .get(key)
    }

    pub fn calculate_number_of_trades(&self, loot_context: &mut LootContext) -> i32 {
        self.amount.value().get_int(loot_context)
    }
}
