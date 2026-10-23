use crate::world::item::component::types::{self, Pair, record, req};

pub fn codec() -> Pair {
    record(vec![req("asset_id", types::identifier())])
}
