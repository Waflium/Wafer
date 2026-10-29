use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::levelgen::density_function::holder_codec;

pub const TYPES: [&str; 6] = ["minecraft:add", "minecraft:sub", "minecraft:mul", "minecraft:div", "minecraft:min", "minecraft:max"];

pub fn fields() -> Vec<Field> {
    vec![req("left", holder_codec()), req("right", holder_codec())]
}
