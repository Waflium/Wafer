use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, mob_effects};

use super::wither_boss::{self, WitherBoss};
use crate::server::level::{ServerLevel, testing};
use crate::world::difficulty::Difficulty;
use crate::world::effect::mob_effect_instance::MobEffectInstance;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::BlockGetter;
use crate::world::level::block::wither_skull_block;
use crate::world::level::update_flags::UPDATE_ALL;

fn withers(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.values().filter_map(|e| match e {
        LevelEntity::Mob(mob) if level.is_wither(mob.entity.id) && !mob.entity.is_removed() => Some(mob.entity.id),
        _ => None,
    }).collect()
}

fn build_wither(level: &mut ServerLevel) {
    for pos in [BlockPos::new(1, -60, 0), BlockPos::new(0, -59, 0), BlockPos::new(1, -59, 0), BlockPos::new(2, -59, 0)] {
        level.set_block(pos, blocks::SOUL_SAND.default_state(), UPDATE_ALL);
    }
    for x in 0..3 {
        level.set_block(BlockPos::new(x, -58, 0), blocks::WITHER_SKELETON_SKULL.default_state(), UPDATE_ALL);
    }
    wither_skull_block::check_spawn(level, BlockPos::new(2, -58, 0));
}

#[test]
fn building_the_pattern_summons_an_invulnerable_wither() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    build_wither(&mut level);
    let found = withers(&level);
    assert_eq!(found.len(), 1);
    let id = found[0];
    assert_eq!(level.entity(id).position(), Vec3::new(1.5, -59.45, 0.5));
    assert_eq!(wither_boss::get_invulnerable_ticks(level.entity(id)), 220);
    assert_eq!(level.mob(id).unwrap().health, 100.0);
    assert!(level.block_state(BlockPos::new(1, -59, 0)).is_air());
    assert!(level.block_state(BlockPos::new(1, -58, 0)).is_air());
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    assert!(out.iter().any(|(k, v)| k == "Invul" && *v == wafer_protocol::nbt::Nbt::Int(220)));
}

#[test]
fn withers_ignore_effects_and_count_down_their_invulnerability() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    build_wither(&mut level);
    let id = withers(&level)[0];
    assert!(!level.add_effect(id, MobEffectInstance::new(mob_effects::SPEED, 100, 0), None));
    for _ in 0..10 {
        level.tick();
    }
    assert_eq!(wither_boss::get_invulnerable_ticks(level.entity(id)), 210);
    assert!(level.mob(id).unwrap().health > 100.0);
}

#[test]
fn peaceful_prevents_withers() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Peaceful;
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    build_wither(&mut level);
    assert!(withers(&level).is_empty());
    let mut mob = WitherBoss::create();
    mob.entity.snap_to(0.5, -60.0, 0.5, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    level.tick();
    assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()));
}
