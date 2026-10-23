use crate::world::item::component::types::{self, Pair, record, req};

pub fn direct_codec() -> Pair {
    record(vec![req("asset_id", types::identifier()), req("translation_key", types::string())])
}
