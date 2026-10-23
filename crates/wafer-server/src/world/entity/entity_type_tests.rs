use wafer_core::Vec3;

use crate::server::level::testing;
use crate::world::entity::LevelEntity;
use crate::world::entity::animal::cow::cow::Cow;
use crate::world::entity::decoration::item_frame::ItemFrame;
use crate::world::entity::entity_type::EntityType;

#[test]
fn default_loot_tables_match_vanilla() {
    let mut checked = 0;
    for line in include_str!("entity_default_loot_table_oracle.txt").lines() {
        let (key, table) = line.split_once(' ').unwrap();
        let expected = (table != "NONE").then(|| table.to_string());
        assert_eq!(EntityType::by_name(key).unwrap().get_default_loot_table(), expected, "{key}");
        checked += 1;
    }
    assert_eq!(checked, wafer_data::registry::ENTITY_TYPE.len());
}

#[test]
fn level_loot_tables_follow_the_entity_type() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    assert_eq!(level.get_loot_table(player).as_deref(), Some("minecraft:entities/player"));

    let mut cow = Cow::create();
    let cow_id = cow.entity.id;
    cow.loot_table = Some("minecraft:entities/zombie".to_string());
    let other_cow = Cow::create();
    let other_cow_id = other_cow.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(cow)));
    level.add_fresh_entity(LevelEntity::Mob(Box::new(other_cow)));
    assert_eq!(level.get_loot_table(cow_id).as_deref(), Some("minecraft:entities/zombie"));
    assert_eq!(level.get_loot_table(other_cow_id).as_deref(), Some("minecraft:entities/cow"));

    let frame = ItemFrame::create_empty(false);
    let frame_id = frame.entity.id;
    level.add_fresh_entity(LevelEntity::ItemFrame(frame));
    assert_eq!(level.get_loot_table(frame_id), None);
}
