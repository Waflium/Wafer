use crate::world::item::component::codec::{Codec, Field, RangeError, Stream};
use crate::world::item::component::types::{self, req};
use crate::world::level::levelgen::material::vertical_anchor_codec;

pub fn fields() -> Vec<Field> {
    vec![
        req("anchor", vertical_anchor_codec()),
        req("surface_depth_multiplier", (Codec::Int(-20, 20, RangeError::Outside), Stream::Int)),
        req("add_stone_depth", types::bool()),
    ]
}
