use crate::world::item::component::codec::{Codec, Stream};
use crate::world::item::component::types::{self, Pair, record, req};

pub fn codec() -> Pair {
    record(vec![
        req("sound", types::sound_event()),
        req("tick_delay", (Codec::int(), Stream::Int)),
        req("block_search_extent", (Codec::int(), Stream::Int)),
        req("offset", (Codec::Double, Stream::Double)),
    ])
}
