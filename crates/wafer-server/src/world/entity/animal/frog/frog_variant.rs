use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::entity::variant::spawn_priority_selectors;

pub fn direct_codec() -> Pair {
    record(vec![req("asset_id", types::identifier()), req("spawn_conditions", spawn_priority_selectors::codec())])
}

pub fn network_codec() -> Pair {
    record(vec![req("asset_id", types::identifier())])
}
