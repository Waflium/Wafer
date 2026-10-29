use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::req;

pub fn fields() -> Vec<Field> {
    vec![req("max_water_depth", (Codec::int(), Stream::Int))]
}
