use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::{self, def, req};
use crate::world::level::chunk::HeightmapType;

pub fn fields() -> Vec<Field> {
    vec![
        req("heightmap", HeightmapType::codec()),
        def("min_inclusive", (Codec::int(), Stream::Int), types::i(i32::MIN)),
        def("max_inclusive", (Codec::int(), Stream::Int), types::i(i32::MAX)),
    ]
}
