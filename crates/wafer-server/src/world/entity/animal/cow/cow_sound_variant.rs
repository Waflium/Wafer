use crate::world::item::component::types::{self, Pair, record, req};

pub fn direct_codec() -> Pair {
    record(vec![
        req("ambient_sound", types::sound_event()),
        req("hurt_sound", types::sound_event()),
        req("death_sound", types::sound_event()),
        req("step_sound", types::sound_event()),
    ])
}
