use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::levelgen::density_function::holder_codec;

pub fn fields() -> Vec<Field> {
    vec![req("alpha", holder_codec()), req("first", holder_codec()), req("second", holder_codec())]
}
