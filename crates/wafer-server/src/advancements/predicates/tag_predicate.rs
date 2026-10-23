use wafer_protocol::nbt::Nbt;

use crate::core::holder_set::HolderSet;
use crate::util::codec::{self, DataResult};
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
        Ok(TagPredicate { tag: codec::decode_field(tag, "id", |t| HolderSet::decode(registry, t))?, expected: codec::decode_field(tag, "expected", codec::bool_value)? })
    }

    pub fn matches(&self, id: i32) -> bool {
        self.tag.contains(id) == self.expected
    }
}
