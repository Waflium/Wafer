use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![req("biome_is", types::holder_set("minecraft:worldgen/biome"))]
}
