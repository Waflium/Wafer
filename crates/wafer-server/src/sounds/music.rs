use crate::world::item::component::types::{self, Pair, def, record, req};

pub fn codec() -> Pair {
    record(vec![
        req("sound", types::sound_event()),
        req("min_delay", types::var_int(types::non_negative_int())),
        req("max_delay", types::var_int(types::non_negative_int())),
        def("replace_current_music", types::bool(), types::b(false)),
    ])
}
