use crate::world::item::component::codec::Field;
use crate::world::item::component::types::{self, req};
use crate::world::level::block::state::block_state;
use crate::world::level::levelgen::density_function;

pub fn fields() -> Vec<Field> {
    vec![
        req("ore_block", block_state::codec()),
        req("raw_ore_block", block_state::codec()),
        req("filler_block", block_state::codec()),
        req("raw_ore_chance", types::float(types::codec_float_range(0.0, 1.0))),
        req("density", density_function::holder_codec()),
        req("richness", density_function::holder_codec()),
        req("filler_gap", density_function::holder_codec()),
    ]
}
