use wafer_core::Vec3;
use wafer_data::damage_types;

use super::breeze::Breeze;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::data::Pose;
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{EntityId, LevelEntity};

fn add_breeze(level: &mut ServerLevel, x: f64, z: f64) -> EntityId {
    let mut mob = Breeze::create();
    mob.entity.snap_to(x, -60.0, z, 0.0, 0.0);
    mob.persistence_required = true;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

#[test]
fn breezes_attack_players_with_wind_charges() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let breeze = add_breeze(&mut level, 0.5, 10.5);
    let mut targeted = false;
    let mut shot = false;
    let mut poses = Vec::new();
    for _ in 0..200 {
        level.tick();
        targeted |= level.get_brain(breeze).get_memory(memory_module_type::ATTACK_TARGET) == Some(&player);
        shot |= level.entities.values().any(|e| matches!(e, LevelEntity::Projectile(p) if matches!(p.kind, ProjectileKind::BreezeWindCharge(_))));
        let pose = level.entity(breeze).pose();
        if poses.last() != Some(&pose) {
            poses.push(pose);
        }
    }
    assert!(targeted, "the breeze never targeted the player");
    assert!(shot, "the breeze never shot a wind charge");
    assert!(poses.contains(&Pose::Shooting) && poses.contains(&Pose::Inhaling) && poses.contains(&Pose::LongJumping), "{poses:?}");
}

#[test]
fn breezes_ignore_damage_from_other_breezes() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let a = add_breeze(&mut level, 20.5, 20.5);
    let b = add_breeze(&mut level, 22.5, 20.5);
    let source = DamageSource::entity(damage_types::MOB_ATTACK, b);
    assert!(level.breeze_is_invulnerable_to(a, &source));
    assert!(!level.breeze_can_attack(a, b));
}
