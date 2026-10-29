use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::chunk::HeightmapType;

pub fn fields() -> Vec<Field> {
    vec![req("heightmap", HeightmapType::codec())]
}
