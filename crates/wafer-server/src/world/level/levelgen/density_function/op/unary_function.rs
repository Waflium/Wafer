use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::levelgen::density_function::holder_codec;

pub const TYPES: [&str; 11] = [
    "minecraft:abs",
    "minecraft:square",
    "minecraft:cube",
    "minecraft:sqrt",
    "minecraft:half_negative",
    "minecraft:quarter_negative",
    "minecraft:reciprocal",
    "minecraft:negate",
    "minecraft:squeeze",
    "minecraft:log",
    "minecraft:sign",
];

pub fn fields() -> Vec<Field> {
    vec![req("input", holder_codec())]
}
