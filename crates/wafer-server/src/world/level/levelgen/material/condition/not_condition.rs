use super::material_condition;
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;

pub fn fields() -> Vec<Field> {
    vec![req("invert", material_condition::holder_codec())]
}
