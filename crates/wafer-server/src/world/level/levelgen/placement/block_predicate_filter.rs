use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::levelgen::blockpredicates::block_predicate;

pub fn fields() -> Vec<Field> {
    vec![req("predicate", block_predicate::codec())]
}
