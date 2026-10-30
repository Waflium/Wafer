use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::endermite::Endermite;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::Mob;
use crate::world::entity::{EntityId, LevelEntity};

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn level() -> ServerLevel {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 40.5), 0.0, 0.0);
    level
}

fn endermite_with_life(life: i32, persistent: bool) -> Mob {
    let mut mob = Endermite::create();
    Endermite::read_additional_save_data(&mut mob, &Nbt::Compound(vec![("Lifetime".into(), Nbt::Int(life))].into()));
    mob.persistence_required = persistent;
    mob
}

fn lifetime(mob: &Mob) -> Option<i32> {
    let mut out = Vec::new();
    mob.save(&mut out);
    out.into_iter().find(|(key, _)| key == "Lifetime").and_then(|(_, tag)| match tag {
        Nbt::Int(life) => Some(life),
        _ => None,
    })
}

#[test]
fn endermites_have_vanillas_attributes() {
    let mob = Endermite::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 8.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.25);
    assert_eq!(attributes.value(Attribute::AttackDamage), 2.0);
    assert_eq!(mob.xp_reward, 3);
    assert_eq!(lifetime(&mob), Some(0));
}

#[test]
fn endermites_despawn_after_their_lifetime() {
    let mut level = level();
    let mortal = add(&mut level, endermite_with_life(2390, false), Vec3::new(0.5, -60.0, 0.5));
    let persistent = add(&mut level, endermite_with_life(2390, true), Vec3::new(4.5, -60.0, 0.5));
    let mut ticks = 0;
    while level.entities.contains_key(&mortal) {
        level.tick();
        ticks += 1;
        assert!(ticks <= 10, "the endermite outlived its lifetime");
    }
    assert_eq!(ticks, 10);
    let LevelEntity::Mob(mob) = &level.entities[&persistent] else { unreachable!() };
    assert_eq!(lifetime(mob), Some(2390));
}

#[test]
fn endermite_lifetime_survives_saving() {
    let mut level = level();
    let id = add(&mut level, endermite_with_life(100, false), Vec3::new(0.5, -60.0, 0.5));
    for _ in 0..5 {
        level.tick();
    }
    let LevelEntity::Mob(mob) = &level.entities[&id] else { unreachable!() };
    assert_eq!(lifetime(mob), Some(105));
    let mut saved = Vec::new();
    mob.save(&mut saved);
    let mut loaded = Endermite::create();
    loaded.load(&Nbt::Compound(saved.into()));
    assert_eq!(lifetime(&loaded), Some(105));
}
