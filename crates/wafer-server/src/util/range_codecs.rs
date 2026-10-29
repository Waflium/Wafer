use crate::world::item::component::codec::{Codec, RangeError, Stream};
use crate::world::item::component::types::{self, Pair};

pub fn int_range(min: i32, max: i32) -> Pair {
    (Codec::Int(min, max, RangeError::Outside), Stream::Int)
}

pub fn float_range(min: f32, max: f32) -> Pair {
    types::float(types::codec_float_range(min, max))
}

pub fn double_range(min: f64, max: f64) -> Pair {
    (Codec::DoubleRange(min, max), Stream::Double)
}

pub fn int() -> Pair {
    (Codec::int(), Stream::Int)
}

pub fn double() -> Pair {
    (Codec::Double, Stream::Double)
}
