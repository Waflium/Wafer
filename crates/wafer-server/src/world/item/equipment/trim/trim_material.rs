use crate::world::item::component::types::{self, Pair, record, req};
use crate::world::item::component::text;

pub fn direct_codec() -> Pair {
    record(vec![req("palette_id", types::identifier()), req("description", text::text())])
}
