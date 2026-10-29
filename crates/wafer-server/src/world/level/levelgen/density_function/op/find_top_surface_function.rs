use crate::world::item::component::codec::{Codec, Field, RangeError, Stream};
use crate::world::item::component::types::{self, req};
use crate::world::level::dimension::dimension_type::{MAX_Y, MIN_Y};
use crate::world::level::levelgen::density_function::holder_codec;

pub fn fields() -> Vec<Field> {
    vec![
        req("density", holder_codec()),
        req("upper_bound", holder_codec()),
        req("lower_bound", (Codec::Int(MIN_Y * 2, MAX_Y * 2, RangeError::Outside), Stream::Int)),
        req("cell_height", types::var_int(types::positive_int())),
    ]
}
