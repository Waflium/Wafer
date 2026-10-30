use wafer_protocol::nbt::Nbt;

use super::data_component_matchers::DataComponentMatchers;
use super::min_max_bounds::Ints;
use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::ItemStack;

#[derive(Debug)]
pub struct ItemPredicate {
    items: Option<HolderSet>,
    count: Ints,
    components: DataComponentMatchers,
}

impl ItemPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<ItemPredicate> {
        ItemPredicate::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<ItemPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let items = record.optional(tag, "items", |t| HolderSet::decode_partial("minecraft:item", t));
            let count = record.with_default(tag, "count", Ints::ANY, Ints::decode_partial);
            let components = record.nested(|| DataComponentMatchers::decode_partial(tag));
            record.build(|| Some(ItemPredicate { items, count, components: components? }))
        })
    }

    pub fn test(&self, item_stack: &ItemStack) -> bool {
        if self.items.as_ref().is_some_and(|items| !items.contains(item_stack.item.0 as i32)) {
            return false;
        }
        self.count.matches(item_stack.count) && self.components.test(item_stack)
    }
}
