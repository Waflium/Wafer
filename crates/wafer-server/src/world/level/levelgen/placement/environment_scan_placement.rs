use crate::core::direction;
use crate::world::item::component::codec::{Codec, Field, RangeError, Stream};
use crate::world::item::component::types::{self, def, req};
use crate::world::level::levelgen::blockpredicates::block_predicate;

pub fn fields() -> Vec<Field> {
    vec![
        req("direction_of_search", direction::vertical_codec()),
        req("target_condition", block_predicate::codec()),
        def("allowed_search_condition", block_predicate::codec(), types::compound(&[("type", types::s("minecraft:true"))])),
        req("max_steps", (Codec::Int(1, 32, RangeError::Outside), Stream::Int)),
    ]
}
