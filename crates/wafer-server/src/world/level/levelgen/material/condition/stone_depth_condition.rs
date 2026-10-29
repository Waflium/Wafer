use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![
        req("offset", (Codec::int(), Stream::Int)),
        req("add_surface_depth", types::bool()),
        req("secondary_depth_range", (Codec::int(), Stream::Int)),
        req("surface_type", types::enumeration(&["ceiling", "floor"])),
    ]
}
