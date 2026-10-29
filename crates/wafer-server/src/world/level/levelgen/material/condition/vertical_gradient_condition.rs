use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};
use crate::world::level::levelgen::material::vertical_anchor_codec;

pub fn fields() -> Vec<Field> {
    vec![
        req("random_name", types::identifier()),
        req("true_at_and_below", vertical_anchor_codec()),
        req("false_at_and_above", vertical_anchor_codec()),
    ]
}
