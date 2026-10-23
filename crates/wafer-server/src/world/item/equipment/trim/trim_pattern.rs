use crate::world::item::component::types::{self, Pair, always, record, req};
use crate::world::item::component::text;

pub fn direct_codec() -> Pair {
    record(vec![req("asset_id", types::identifier()), req("description", text::text()), always("decal", types::bool(), types::b(false))])
}
