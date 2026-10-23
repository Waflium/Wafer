use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult};
use crate::world::item::component::types::{self, Pair};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct InclusiveRange {
    pub min_inclusive: i32,
    pub max_inclusive: i32,
}

impl InclusiveRange {
    pub const fn new(min_inclusive: i32, max_inclusive: i32) -> InclusiveRange {
        InclusiveRange { min_inclusive, max_inclusive }
    }

    pub fn create(min_inclusive: i32, max_inclusive: i32) -> DataResult<InclusiveRange> {
        if min_inclusive <= max_inclusive {
            Ok(InclusiveRange::new(min_inclusive, max_inclusive))
        } else {
            Err("min_inclusive must be less than or equal to max_inclusive".into())
        }
    }

    pub fn is_value_in_range(&self, value: i32) -> bool {
        value >= self.min_inclusive && value <= self.max_inclusive
    }

    pub fn contains(&self, sub_range: &InclusiveRange) -> bool {
        sub_range.min_inclusive >= self.min_inclusive && sub_range.max_inclusive <= self.max_inclusive
    }

    pub fn decode_int(tag: &Nbt) -> DataResult<InclusiveRange> {
        if let Ok(min) = codec::int(tag) {
            return InclusiveRange::create(min, min);
        }
        let array = codec::list_of(tag, codec::int).and_then(|list| match list[..] {
            [min, max] => InclusiveRange::create(min, max),
            _ => Err(format!("Input is not a list of 2 elements: {list:?}")),
        });
        array.or_else(|array_error| {
            let min = codec::decode_field(tag, "min_inclusive", codec::int).map_err(|_| array_error.clone())?;
            let max = codec::decode_field(tag, "max_inclusive", codec::int).map_err(|_| array_error)?;
            InclusiveRange::create(min, max)
        })
    }

    pub fn int_codec() -> Pair {
        types::transform(|tag| InclusiveRange::decode_int(&tag).map(|range| range.encode_int()))
    }

    pub fn encode_int(&self) -> Nbt {
        if self.min_inclusive == self.max_inclusive {
            Nbt::Int(self.min_inclusive)
        } else {
            Nbt::List(vec![Nbt::Int(self.min_inclusive), Nbt::Int(self.max_inclusive)])
        }
    }
}

impl std::fmt::Display for InclusiveRange {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}, {}]", self.min_inclusive, self.max_inclusive)
    }
}
