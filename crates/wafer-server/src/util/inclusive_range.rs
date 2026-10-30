use wafer_protocol::nbt::Nbt;

use crate::util::codec::{self, DataResult, DecodeResult, Record, either_result};
use crate::world::item::component::codec::Wrap;
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
        let array = || {
            codec::wrapped(Wrap::FlatXmap, || {
                codec::list_partial(tag, |element| DecodeResult::from(codec::int(element))).flat_map(|list| {
                    let fixed = match list.len() {
                        2 => DecodeResult::success(list),
                        size if size > 2 => DecodeResult { value: Some(list[..2].to_vec()), error: Some("Input is not a list of 2 elements".to_string()) },
                        _ => DecodeResult::error("Input is not a list of 2 elements"),
                    };
                    fixed.flat_map(|list| DecodeResult::from(InclusiveRange::create(list[0], list[1])))
                })
            })
        };
        let object = || match codec::map(tag) {
            Err(error) => DecodeResult::error(error),
            Ok(_) => codec::wrapped(Wrap::FlatXmap, || {
                codec::as_codec(|| {
                    let mut record = Record::default();
                    let min = record.required(tag, "min_inclusive", codec::int);
                    let max = record.required(tag, "max_inclusive", codec::int);
                    record.build(|| Some((min?, max?)))
                })
                .flat_map(|(min, max)| DecodeResult::from(InclusiveRange::create(min, max)))
            }),
        };
        let array_or_object = || {
            codec::wrapped(Wrap::Xmap, || {
                either_result(codec::either_first(array), || codec::either_second(object)).map(|either| either.unwrap_or_else(|range| range))
            })
        };
        codec::wrapped(Wrap::FlatXmap, || {
            either_result(codec::either_first(|| DecodeResult::from(codec::int(tag))), || codec::either_second(array_or_object)).flat_map(|either| match either {
                Ok(min) => DecodeResult::from(InclusiveRange::create(min, min)),
                Err(range) => DecodeResult::success(range),
            })
        })
        .into_result()
    }

    pub fn int_codec_in<const MIN: i32, const MAX: i32>() -> Pair {
        types::transform(|tag| {
            codec::wrapped(Wrap::FlatXmap, || {
                let range = InclusiveRange::decode_int(&tag)?;
                if range.min_inclusive < MIN {
                    return Err(format!("Range limit too low, expected at least {MIN} [{}-{}]", range.min_inclusive, range.max_inclusive));
                }
                if range.max_inclusive > MAX {
                    return Err(format!("Range limit too high, expected at most {MAX} [{}-{}]", range.min_inclusive, range.max_inclusive));
                }
                Ok(range.encode_int())
            })
        })
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
