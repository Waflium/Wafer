use wafer_core::Vec3;
use wafer_data::items;

use super::ominous_item_spawner::OminousItemSpawner;
use super::projectile::hurtingprojectile::dragon_fireball::DragonFireball;
use crate::server::level::testing;
use crate::world::entity::LevelEntity;
use crate::world::item::ItemStack;

#[test]
fn ominous_item_spawners_drop_their_item_and_vanish() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(10.5, -60.0, 10.5), 0.0, 0.0);
    let mut spawner = OminousItemSpawner::create(&mut level, ItemStack::new(items::DIAMOND, 2));
    spawner.entity.set_pos(Vec3::new(0.5, -58.0, 0.5));
    let id = spawner.entity.id;
    level.add_fresh_entity(LevelEntity::OminousItemSpawner(Box::new(spawner)));
    for _ in 0..121 {
        level.tick();
    }
    assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()));
    let dropped: Vec<i32> = level.entities.values().filter_map(|e| match e {
        LevelEntity::Item(item) if item.item.is(items::DIAMOND) => Some(item.item.count),
        _ => None,
    }).collect();
    assert_eq!(dropped, vec![2]);
}

#[test]
fn dragon_fireballs_leave_a_breath_cloud() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(10.5, -60.0, 10.5), 0.0, 0.0);
    let mut fireball = DragonFireball::of_type();
    fireball.entity.set_pos(Vec3::new(0.5, -58.0, 0.5));
    fireball.entity.set_delta_movement(Vec3::new(0.0, -0.1, 0.0));
    level.add_fresh_entity(LevelEntity::Projectile(Box::new(fireball)));
    let mut cloud = false;
    for _ in 0..60 {
        level.tick();
        cloud |= level.entities.values().any(|e| matches!(e, LevelEntity::AreaEffectCloud(_)));
    }
    assert!(cloud, "the fireball never splashed");
    let cloud_id = level.entities.values().find_map(|e| match e {
        LevelEntity::AreaEffectCloud(cloud) => Some(cloud.entity.id),
        _ => None,
    }).unwrap();
    let mut out = Vec::new();
    level.save_without_id(cloud_id, &mut out);
    let particle = out.iter().find(|(k, _)| k == "custom_particle").map(|(_, v)| v.clone()).unwrap();
    assert_eq!(particle.get("power"), Some(&wafer_protocol::nbt::Nbt::Float(1.0)));
}
