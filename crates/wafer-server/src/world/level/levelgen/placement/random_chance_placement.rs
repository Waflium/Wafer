use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![req("chance", types::float(types::codec_float_range(0.0, 1.0)))]
}
