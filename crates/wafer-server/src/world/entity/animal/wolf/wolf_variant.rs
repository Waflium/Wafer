use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::entity::variant::spawn_priority_selectors;

fn asset_info() -> Pair {
    record(vec![req("wild", types::identifier()), req("tame", types::identifier()), req("angry", types::identifier())])
}

pub fn direct_codec() -> Pair {
    record(vec![req("assets", asset_info()), req("baby_assets", asset_info()), req("spawn_conditions", spawn_priority_selectors::codec())])
}

pub fn network_codec() -> Pair {
    record(vec![req("assets", asset_info()), req("baby_assets", asset_info())])
}
