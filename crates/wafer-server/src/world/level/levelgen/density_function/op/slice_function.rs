use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::{self, req};
use crate::world::level::levelgen::density_function::{density_functions, holder_codec};

pub fn fields() -> Vec<Field> {
    vec![req("axis", types::enumeration(density_functions::AXES)), req("coordinate", (Codec::int(), Stream::Int)), req("input", holder_codec())]
}
