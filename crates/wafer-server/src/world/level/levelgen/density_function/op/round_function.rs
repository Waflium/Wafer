use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, def, req};
use crate::world::level::levelgen::density_function::holder_codec;

pub const TYPES: [&str; 4] = ["minecraft:floor", "minecraft:round", "minecraft:ceil", "minecraft:truncate"];

pub fn fields() -> Vec<Field> {
    vec![req("input", holder_codec()), def("multiple", holder_codec(), types::f(1.0))]
}
