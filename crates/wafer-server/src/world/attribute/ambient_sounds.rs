use super::{ambient_additions_settings, ambient_mood_settings};
use crate::world::item::component::types::{self, Pair, def, opt, record};

pub fn codec() -> Pair {
    record(vec![
        opt("loop", types::sound_event()),
        opt("mood", ambient_mood_settings::codec()),
        def("additions", types::compact_list(ambient_additions_settings::codec()), types::empty_list()),
    ])
}
