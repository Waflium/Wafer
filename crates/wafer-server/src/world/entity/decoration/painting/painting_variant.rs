use std::sync::LazyLock;

use serde_json::Value;
use wafer_data::SyncedRegistry;

pub const REGISTRY: &str = "minecraft:painting_variant";
pub const PLACEABLE: &str = "minecraft:placeable";

pub struct PaintingVariant {
    pub width: i32,
    pub height: i32,
}

impl PaintingVariant {
    pub fn area(&self) -> i32 {
        self.width * self.height
    }
}

static VARIANTS: LazyLock<Vec<PaintingVariant>> = LazyLock::new(|| {
    SyncedRegistry::get(REGISTRY)
        .entries
        .iter()
        .map(|name| {
            let json = wafer_data::PAINTING_VARIANTS.iter().find(|(id, _)| id == name).map(|(_, json)| *json).expect("vanilla painting variant");
            let json: Value = serde_json::from_str(json).expect("vanilla JSON");
            PaintingVariant { width: json["width"].as_i64().unwrap_or(1) as i32, height: json["height"].as_i64().unwrap_or(1) as i32 }
        })
        .collect()
});

pub fn get(id: i32) -> &'static PaintingVariant {
    &VARIANTS[id as usize]
}
