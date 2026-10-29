use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::req;

fn scale_range() -> (Codec, Stream) {
    (Codec::DoubleRange(0.001, 1000.0), Stream::Double)
}

pub fn fields() -> Vec<Field> {
    vec![
        req("xz_scale", scale_range()),
        req("y_scale", scale_range()),
        req("xz_factor", scale_range()),
        req("y_factor", scale_range()),
        req("smear_scale_multiplier", (Codec::DoubleRange(1.0, 8.0), Stream::Double)),
    ]
}
