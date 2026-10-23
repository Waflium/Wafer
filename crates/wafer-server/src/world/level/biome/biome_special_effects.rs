use crate::util::extra_codecs::string_rgb_color;
use crate::world::item::component::types::{self, Pair, def, opt, record, req};

pub fn codec() -> Pair {
    record(vec![
        req("water_color", string_rgb_color()),
        opt("foliage_color", string_rgb_color()),
        opt("dry_foliage_color", string_rgb_color()),
        opt("grass_color", string_rgb_color()),
        def("grass_color_modifier", types::enumeration(&["none", "dark_forest", "swamp"]), types::s("none")),
    ])
}
