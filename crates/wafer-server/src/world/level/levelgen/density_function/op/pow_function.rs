use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::levelgen::density_function::holder_codec;

pub fn fields() -> Vec<Field> {
    vec![req("base", holder_codec()), req("exponent", holder_codec())]
}
