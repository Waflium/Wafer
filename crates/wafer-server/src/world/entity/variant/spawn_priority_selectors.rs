use crate::world::item::component::types::{self, Pair};
use super::priority_provider;
use super::spawn_condition;

pub fn codec() -> Pair {
    types::list(priority_provider::selector_codec(spawn_condition::codec()))
}
