use crate::world::item::component::codec::{Codec, Field, RangeError, Stream};
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![
        req("offset", (Codec::int(), Stream::Int)),
        req("surface_depth_multiplier", (Codec::Int(-20, 20, RangeError::Outside), Stream::Int)),
        req("add_stone_depth", types::bool()),
    ]
}
