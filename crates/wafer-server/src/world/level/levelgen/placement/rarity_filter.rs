use crate::world::item::component::codec::{Field, Stream};
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![req("chance", (types::positive_int(), Stream::Int))]
}
