use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::{self, def, req};

pub fn fields() -> Vec<Field> {
    vec![
        req("noise", types::identifier()),
        req("min_threshold", (Codec::Double, Stream::Double)),
        req("max_threshold", (Codec::Double, Stream::Double)),
        def("is_3d", types::bool(), Nbt::Byte(0)),
    ]
}
