use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};
use crate::world::level::levelgen::density_function::holder_codec;

pub fn fields() -> Vec<Field> {
    vec![
        req("input", holder_codec()),
        req("cell_size_xz", types::var_int(types::positive_int())),
        req("cell_size_y", types::var_int(types::positive_int())),
    ]
}
