use crate::world::item::component::types::{Pair, record, req};
use crate::world::entity::variant::{model_and_texture, spawn_priority_selectors};

pub fn direct_codec() -> Pair {
    let mut fields = model_and_texture::fields(&["normal", "warm"]);
    fields.push(req("spawn_conditions", spawn_priority_selectors::codec()));
    record(fields)
}

pub fn network_codec() -> Pair {
    record(model_and_texture::fields(&["normal", "warm"]))
}
