use wafer_data::Item;
use wafer_protocol::nbt::Nbt;

use super::item_cost::{ItemCost, decode_item};
use crate::core::component::data_component_exact_predicate::DataComponentExactPredicate;
use crate::core::holder::Holder;
use crate::util::codec::{self, DecodeResult};
use crate::world::item::ItemStack;
use crate::world::level::storage::loot::loot_context::LootContext;
use crate::world::level::storage::loot::providers::number::ints::{self, ContextIntProvider};
use crate::world::level::storage::loot::validatable::{self, Validatable};
use crate::world::level::storage::loot::validation_context::ValidationContext;

pub struct TradeCost {
    pub item: Item,
    pub count: Holder<ContextIntProvider>,
    pub components: DataComponentExactPredicate,
}

impl TradeCost {
    pub fn decode(tag: &Nbt) -> DecodeResult<TradeCost> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        codec::as_codec(|| {
            let mut record = codec::Record::default();
            let item = record.required(tag, "id", decode_item);
            let count = record.with_default(tag, "count", ints::exactly(1), ContextIntProvider::decode_holder);
            let components = record.with_default(tag, "components", DataComponentExactPredicate::default(), DataComponentExactPredicate::decode);
            record.build(|| Some(TradeCost { item: item?, count, components }))
        })
    }

    pub fn to_item_cost(&self, loot_context: &mut LootContext, additional_cost: i32) -> ItemCost {
        let max = ItemStack::new(self.item, 1).max_stack_size();
        let count = (self.count.value().get_int(loot_context) + additional_cost).clamp(0, max);
        ItemCost::new(self.item, count, self.components.clone())
    }
}

impl Validatable for TradeCost {
    fn validate(&self, context: &ValidationContext) {
        validatable::validate_holder(context, "count", &self.count);
    }
}
