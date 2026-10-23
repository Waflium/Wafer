use crate::world::item::component::types::{self, Pair, record, req};

fn sound_set() -> Pair {
    record(vec![
        req("ambient_sound", types::sound_event()),
        req("hurt_sound", types::sound_event()),
        req("death_sound", types::sound_event()),
        req("step_sound", types::sound_event()),
    ])
}

pub fn direct_codec() -> Pair {
    record(vec![req("adult_sounds", sound_set()), req("baby_sounds", sound_set())])
}
