use crate::world::item::component::types::{self, Pair};
use super::priority_provider;
use super::spawn_condition;

pub fn codec() -> Pair {
    let (list, stream) = types::list(priority_provider::selector_codec(spawn_condition::codec()));
    (crate::world::item::component::codec::Codec::Framed(Box::new(list), crate::world::item::component::codec::Wrap::Xmap), stream)
}
