use super::world_preset::WorldPreset;
use crate::world::item::component::json_to_nbt;

pub const NORMAL: &str = "minecraft:normal";
pub const FLAT: &str = "minecraft:flat";

pub fn contains(key: &str) -> bool {
    crate::core::data_registries::json_entries("minecraft:worldgen/world_preset").iter().any(|(name, _)| *name == key)
}

pub fn first() -> Option<&'static str> {
    crate::core::data_registries::json_entries("minecraft:worldgen/world_preset").first().map(|(name, _)| *name)
}

pub fn get(key: &str) -> Option<WorldPreset> {
    if let Some(preset) = crate::core::data_registries::decoded_world_preset(key) {
        return Some(preset);
    }
    let (_, json) = crate::core::data_registries::json_entries("minecraft:worldgen/world_preset").iter().find(|(name, _)| *name == key)?;
    let json: serde_json::Value = serde_json::from_str(json).ok()?;
    crate::nbt::with_json_source(|| WorldPreset::decode(&json_to_nbt(&json))).value
}
