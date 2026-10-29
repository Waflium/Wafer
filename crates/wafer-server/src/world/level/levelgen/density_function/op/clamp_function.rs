use wafer_protocol::nbt::Nbt;

use crate::nbt::java_float_to_string;
use crate::world::item::component::codec::{self, Field};
use crate::world::item::component::types::{self, req};
use crate::world::level::levelgen::density_function::{density_functions, holder_codec};

pub fn fields() -> Vec<Field> {
    vec![
        req("input", holder_codec()),
        req("min", types::float(density_functions::noise_value_codec())),
        req("max", types::float(density_functions::noise_value_codec())),
    ]
}

pub fn validate(clamp: &Nbt) -> Result<(), String> {
    let min = codec::as_f32(clamp.get("min").unwrap_or(&Nbt::Float(0.0)));
    let max = codec::as_f32(clamp.get("max").unwrap_or(&Nbt::Float(0.0)));
    if max < min { Err(format!("min ({}) must be less than or equal to max ({})", java_float_to_string(min), java_float_to_string(max))) } else { Ok(()) }
}
