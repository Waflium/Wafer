use wafer_core::Vec3;
use wafer_data::registry;

use crate::server::level::{ServerLevel, testing};

#[test]
fn entity_data_layouts_match_vanilla() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mismatches = Vec::new();
    for line in include_str!("entity_data_layout_oracle.txt").lines() {
        let (name, expected) = line.split_once(' ').unwrap_or((line, ""));
        let created = if name == "minecraft:player" { None } else { ServerLevel::create_entity(name) };
        let data = match (name, &created) {
            ("minecraft:player", _) => &level.entity(player).data,
            (_, Some(entity)) => &entity.entity().data,
            (_, None) => continue,
        };
        let actual: Vec<String> = data.serializer_ids().enumerate().map(|(id, serializer)| format!("{id}:{serializer}")).collect();
        if actual.join(" ") != expected {
            mismatches.push(format!("{name}\n  vanilla {expected}\n  wafer   {}", actual.join(" ")));
        }
    }
    assert!(registry::ENTITY_TYPE.len() > 0);
    assert!(mismatches.is_empty(), "{}", mismatches.join("\n"));
}
