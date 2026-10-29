use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};
use crate::world::level::levelgen::density_function::{density_functions, holder_codec};

pub fn fields() -> Vec<Field> {
    vec![
        req("input", holder_codec()),
        req("min_inclusive", types::float(density_functions::noise_value_codec())),
        req("max_exclusive", types::float(density_functions::noise_value_codec())),
        req("when_in_range", holder_codec()),
        req("when_out_of_range", holder_codec()),
    ]
}
