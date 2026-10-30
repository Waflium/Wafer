use crate::world::item::component::types::{self, def, req};
use crate::world::item::component::codec::Field;

pub fn fields(models: &'static [&'static str]) -> Vec<Field> {
    vec![def("model", types::enumeration(models), types::s(models[0])).grouped(2), req("asset_id", types::identifier()).grouped(2)]
}
