use crate::world::item::component::codec::Field;
use crate::world::level::levelgen::structure::structure::simple_codec;

pub fn fields() -> Vec<Field> {
    simple_codec()
}
