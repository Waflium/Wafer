use std::collections::HashMap;
use std::sync::OnceLock;

use wafer_protocol::nbt::Nbt;

use crate::core::holder::Holder;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult, DecodeResult, Record};
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
        TradeSet::decode_partial(tag).into_result()
    }

    fn decode_partial(tag: &Nbt) -> DecodeResult<TradeSet> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let trades = record.required(tag, "trades", |t| HolderSet::decode_partial(super::villager_trade::REGISTRY, t));
        let amount = record.required(tag, "amount", ContextIntProvider::decode_holder_partial);
        let allow_duplicates = record.with_default(tag, "allow_duplicates", false, codec::bool_value);
        let random_sequence = record.optional(tag, "random_sequence", codec::identifier);
        record.build(|| Some(TradeSet { trades: trades?, amount: amount?, allow_duplicates, random_sequence }))
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
