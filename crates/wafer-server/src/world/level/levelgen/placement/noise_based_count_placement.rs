use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::{def, req};

pub fn fields() -> Vec<Field> {
    vec![
        req("noise_to_count_ratio", (Codec::int(), Stream::Int)),
        req("noise_factor", (Codec::Double, Stream::Double)),
        def("noise_offset", (Codec::Double, Stream::Double), Nbt::Double(0.0)),
    ]
}
