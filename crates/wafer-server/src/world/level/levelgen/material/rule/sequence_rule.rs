use super::material_rule;
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![req("sequence", types::list(material_rule::holder_codec()))]
}
