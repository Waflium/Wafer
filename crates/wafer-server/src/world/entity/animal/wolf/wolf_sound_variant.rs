use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;
use crate::world::item::component::types::{self, Pair, record, req};

pub const REGISTRY: &str = "minecraft:wolf_sound_variant";

pub struct WolfSoundSet {
    pub ambient_sound: String,
    pub death_sound: String,
    pub growl_sound: String,
    pub hurt_sound: String,
    pub pant_sound: String,
    pub step_sound: String,
    pub whine_sound: String,
}

pub struct WolfSoundVariant {
    pub adult_sounds: WolfSoundSet,
    pub baby_sounds: WolfSoundSet,
}

fn sound_set(v: &Value) -> WolfSoundSet {
    let sound = |key: &str| v[key].as_str().unwrap_or_default().to_string();
    WolfSoundSet {
        ambient_sound: sound("ambient_sound"),
        death_sound: sound("death_sound"),
        growl_sound: sound("growl_sound"),
        hurt_sound: sound("hurt_sound"),
        pant_sound: sound("pant_sound"),
        step_sound: sound("step_sound"),
        whine_sound: sound("whine_sound"),
    }
}

pub fn sound_variant(id: i32) -> &'static WolfSoundVariant {
    static SOUND_VARIANTS: OnceLock<Vec<WolfSoundVariant>> = OnceLock::new();
    let all = SOUND_VARIANTS.get_or_init(|| {
        let json: HashMap<&str, Value> = crate::core::data_registries::json_entries("minecraft:wolf_sound_variant").iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        wafer_data::SyncedRegistry::get(REGISTRY)
            .entries
            .iter()
            .map(|name| WolfSoundVariant { adult_sounds: sound_set(&json[name]["adult_sounds"]), baby_sounds: sound_set(&json[name]["baby_sounds"]) })
            .collect()
    });
    &all[id as usize]
}

fn sound_set_codec() -> Pair {
    record(vec![
        req("ambient_sound", types::sound_event()),
        req("death_sound", types::sound_event()),
        req("growl_sound", types::sound_event()),
        req("hurt_sound", types::sound_event()),
        req("pant_sound", types::sound_event()),
        req("whine_sound", types::sound_event()),
        req("step_sound", types::sound_event()),
    ])
}

pub fn direct_codec() -> Pair {
    record(vec![req("adult_sounds", sound_set_codec()), req("baby_sounds", sound_set_codec())])
}
