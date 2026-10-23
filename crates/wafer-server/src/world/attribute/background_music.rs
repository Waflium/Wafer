use crate::sounds::music;
use crate::world::item::component::types::{Pair, opt, record};

pub fn codec() -> Pair {
    record(vec![opt("default", music::codec()), opt("creative", music::codec()), opt("underwater", music::codec())])
}
