use crate::world::item::component::codec::Field;
use crate::world::item::component::types::req;
use crate::world::level::block::state::block_state;

pub fn fields() -> Vec<Field> {
    vec![req("result_state", block_state::codec())]
}
