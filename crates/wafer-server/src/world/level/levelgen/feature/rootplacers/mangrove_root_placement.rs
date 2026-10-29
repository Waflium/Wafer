use crate::world::item::component::codec::{Codec, RangeError, Stream};
use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::level::levelgen::feature::stateproviders::block_state_provider;

pub fn codec() -> Pair {
    record(vec![
        req("can_grow_through", types::holder_set("minecraft:block")),
        req("muddy_roots_in", types::holder_set("minecraft:block")),
        req("muddy_roots_provider", block_state_provider::holder_codec()),
        req("max_root_width", (Codec::Int(1, 12, RangeError::Outside), Stream::Int)),
        req("max_root_length", (Codec::Int(1, 64, RangeError::Outside), Stream::Int)),
        req("random_skew_chance", types::float(types::codec_float_range(0.0, 1.0))),
    ])
}
