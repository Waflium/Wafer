use crate::util::extra_codecs;
use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};

pub fn fields() -> Vec<Field> {
    vec![req("point", extra_codecs::vec3i_codec()), req("metric", types::enumeration(&["euclidean", "euclidean_squared", "manhattan", "chebyshev"]))]
}
