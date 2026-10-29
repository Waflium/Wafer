use crate::world::item::component::codec::{Codec, Field, Stream};
use crate::world::item::component::types::req;

pub fn fields() -> Vec<Field> {
    vec![
        req("noise_level", (Codec::Double, Stream::Double)),
        req("below_noise", (Codec::int(), Stream::Int)),
        req("above_noise", (Codec::int(), Stream::Int)),
    ]
}
