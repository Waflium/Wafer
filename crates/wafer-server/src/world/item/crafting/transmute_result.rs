use wafer_data::Item;
use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult, DecodeResult, Record};
use crate::world::item::component::DataComponentPatch;
use crate::world::item::component::codec::{Codec, RangeError, as_i32};
use crate::world::item::component::values::{ItemStackTemplate, decode_item};

#[derive(Clone, Debug)]
pub struct TransmuteResult {
    pub item: Option<String>,
    pub count: i32,
    pub components: Nbt,
}

impl TransmuteResult {
    pub fn decode(tag: &Nbt) -> DataResult<TransmuteResult> {
        TransmuteResult::decode_partial(tag).into_result()
    }

    pub fn decode_partial(tag: &Nbt) -> DecodeResult<TransmuteResult> {
        let alternative = || DecodeResult::from(decode_item(tag)).map(|item| TransmuteResult { item: Some(item), count: 1, components: Nbt::Compound(Vec::new().into()) });
        codec::either_result(TransmuteResult::decode_map_partial(tag), alternative).map(|either| match either {
            Ok(result) | Err(result) => result,
        })
    }

    fn decode_map_partial(tag: &Nbt) -> DecodeResult<TransmuteResult> {
        if let Err(error) = codec::map(tag) {
            return DecodeResult::error(error);
        }
        let mut record = Record::default();
        let item = record.optional(tag, "id", |id| DecodeResult::from(decode_item(id)));
        let count = record.with_default(tag, "count", 1, |count| DecodeResult::from(Codec::Int(1, 99, RangeError::Within).decode(count)).map(|count| as_i32(&count)));
        let components = record.with_default(tag, "components", Nbt::Compound(Vec::new().into()), |components| {
            DecodeResult::from(DataComponentPatch::from_nbt(components)).map(|_| components.clone())
        });
        record.build(|| Some(TransmuteResult { item, count, components }))
    }

    pub fn resolve(&self, input_item: Item) -> ItemStackTemplate {
        self.resolve_count(input_item, self.count)
    }

    pub fn resolve_count(&self, input_item: Item, count: i32) -> ItemStackTemplate {
        ItemStackTemplate { item: self.item.clone().unwrap_or_else(|| input_item.name().to_string()), count, components: self.components.clone() }
    }
}
