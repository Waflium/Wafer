use wafer_protocol::nbt::Nbt;

use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::{self, def, req};
use crate::world::level::levelgen::density_function::density_functions;

pub fn fields() -> Vec<Field> {
    vec![
        req("axis", types::enumeration(density_functions::AXES)),
        def("tiling", types::enumeration(&["clamp_to_edge", "repeat", "mirrored_repeat"]), types::s("clamp_to_edge")),
        req("from_coordinate", (Codec::int(), Stream::Int)),
        req("to_coordinate", (Codec::int(), Stream::Int)),
        req("from_value", types::float(density_functions::noise_value_codec())),
        req("to_value", types::float(density_functions::noise_value_codec())),
    ]
}

pub fn validate(gradient: &Nbt) -> Result<(), String> {
    let from = gradient.get("from_coordinate").and_then(Nbt::as_i64);
    let to = gradient.get("to_coordinate").and_then(Nbt::as_i64);
    if from == to { Err("from_coordinate cannot be equal to to_coordinate".into()) } else { Ok(()) }
}
