use wafer_protocol::nbt::Nbt;

use crate::advancements::predicates::collection_predicate;
use crate::world::item::component::codec::Stream;
use crate::world::item::component::custom;
use crate::world::item::component::types::{Pair, opt, record};

fn item_codec() -> Pair {
    (custom::item_predicate(), Stream::Nbt)
}

pub fn codec() -> Pair {
    record(vec![opt("items", collection_predicate::codec(item_codec))])
}

use crate::advancements::predicates::collection_predicate::CollectionPredicate;
use crate::advancements::predicates::item_predicate::ItemPredicate;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::ItemStack;

#[derive(Debug)]
pub struct ContainerPredicate {
    items: Option<CollectionPredicate<ItemPredicate>>,
}

impl ContainerPredicate {
    pub fn decode(tag: &Nbt) -> DataResult<ContainerPredicate> {
        codec::as_codec(|| {
            if let Err(error) = codec::map(tag) {
                return DecodeResult::error(error);
            }
            let mut record = codec::Record::default();
            let items = record.optional(tag, "items", |t| CollectionPredicate::decode(t, ItemPredicate::decode));
            record.build(|| Some(ContainerPredicate { items }))
        })
        .into_result()
    }

    pub fn matches(&self, value: &Nbt) -> bool {
        let Some(items) = &self.items else { return true };
        let non_empty_items: Vec<ItemStack> =
            value.as_list().iter().filter_map(|slot| slot.get("item")).filter_map(|item| ItemStack::from_nbt(item).ok()).filter(|stack| !stack.is_empty()).collect();
        items.test(&non_empty_items, |p, stack| p.test(stack))
    }
}
