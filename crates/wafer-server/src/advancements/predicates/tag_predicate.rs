use wafer_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult, DecodeResult};
use crate::world::item::component::types::{self, Pair, record, req};

pub fn codec(registry: &'static str) -> Pair {
    record(vec![req("id", types::holder_set(registry)), req("expected", types::bool())])
}

#[derive(Debug)]
pub struct TagPredicate {
    tag: HolderSet,
    expected: bool,
}

impl TagPredicate {
    pub fn decode(registry: &'static str, tag: &Nbt) -> DataResult<TagPredicate> {
        codec::map(tag)?;
        let mut record = codec::Record::default();
        let id = record.required(tag, "id", |t| DecodeResult::from(HolderSet::decode(registry, t)));
        let expected = record.required(tag, "expected", |t| DecodeResult::from(codec::bool_value(t)));
        record.build(|| Some(TagPredicate { tag: id?, expected: expected? })).into_result()
    }

    pub fn matches(&self, id: i32) -> bool {
        self.tag.contains(id) == self.expected
    }
}
