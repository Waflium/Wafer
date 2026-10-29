use crate::world::item::component::codec::Field;
use crate::world::item::component::custom;
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![req("positions", types::list(custom::block_pos()))]
}
