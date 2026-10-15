use std::collections::HashMap;
use std::sync::OnceLock;

use serde_json::Value;

pub const REGISTRY: &str = "minecraft:cat_sound_variant";

pub struct CatSoundSet {
    pub ambient_sound: String,
    pub stray_ambient_sound: String,
    pub hiss_sound: String,
    pub hurt_sound: String,
    pub death_sound: String,
    pub eat_sound: String,
    pub beg_for_food_sound: String,
    pub purr_sound: String,
    pub purreow_sound: String,
}

pub struct CatSoundVariant {
    pub adult_sounds: CatSoundSet,
    pub baby_sounds: CatSoundSet,
}

fn sound_set(v: &Value) -> CatSoundSet {
    let sound = |key: &str| v[key].as_str().unwrap_or_default().to_string();
    CatSoundSet {
        ambient_sound: sound("ambient_sound"),
        stray_ambient_sound: sound("stray_ambient_sound"),
        hiss_sound: sound("hiss_sound"),
        hurt_sound: sound("hurt_sound"),
        death_sound: sound("death_sound"),
        eat_sound: sound("eat_sound"),
        beg_for_food_sound: sound("beg_for_food_sound"),
        purr_sound: sound("purr_sound"),
        purreow_sound: sound("purreow_sound"),
    }
}

pub fn sound_variant(id: i32) -> &'static CatSoundVariant {
    static SOUND_VARIANTS: OnceLock<Vec<CatSoundVariant>> = OnceLock::new();
    let all = SOUND_VARIANTS.get_or_init(|| {
        let json: HashMap<&str, Value> = wafer_data::CAT_SOUND_VARIANTS.iter().map(|&(id, json)| (id, serde_json::from_str(json).expect("vanilla JSON"))).collect();
        wafer_data::SyncedRegistry::get(REGISTRY)
            .entries
            .iter()
            .map(|name| CatSoundVariant { adult_sounds: sound_set(&json[name]["adult_sounds"]), baby_sounds: sound_set(&json[name]["baby_sounds"]) })
            .collect()
    });
    &all[id as usize]
}
