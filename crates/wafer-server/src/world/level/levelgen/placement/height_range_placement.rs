use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::levelgen::heightproviders;

pub fn fields() -> Vec<Field> {
    vec![req("height", heightproviders::codec())]
}
