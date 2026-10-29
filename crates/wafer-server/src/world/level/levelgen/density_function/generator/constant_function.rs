use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};
use crate::world::level::levelgen::density_function::density_functions;

pub fn fields() -> Vec<Field> {
    vec![req("value", types::float(density_functions::noise_value_codec()))]
}
