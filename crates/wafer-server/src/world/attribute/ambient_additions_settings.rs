use crate::world::item::component::codec::{Codec, Stream};
use crate::world::item::component::types::{self, Pair, record, req};

pub fn codec() -> Pair {
    record(vec![req("sound", types::sound_event()), req("tick_chance", (Codec::Double, Stream::Double))])
}
