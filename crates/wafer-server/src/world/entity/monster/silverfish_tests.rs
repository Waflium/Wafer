use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::properties::AXIS;
use wafer_data::{blocks, damage_types, game_rule, mob_effects};

use super::silverfish::Silverfish;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::BlockGetter;
use crate::world::level::block::infested_block;
use crate::world::level::update_flags::UPDATE_ALL;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn level() -> ServerLevel {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    testing::player(&mut level, Vec3::new(0.5, -60.0, 40.5), 0.0, 0.0);
    level
}

fn silverfish(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.values().filter(|e| matches!(e, LevelEntity::Mob(mob) if matches!(mob.kind, MobKind::Silverfish(_)))).map(|e| e.entity().id).collect()
}

fn fill(level: &mut ServerLevel, from: BlockPos, to: BlockPos, state: wafer_data::BlockState) {
    for pos in BlockPos::between_closed(from, to) {
        level.set_block(pos, state, UPDATE_ALL);
    }
}

fn pocket(level: &mut ServerLevel, center: BlockPos, state: wafer_data::BlockState) {
    fill(level, center.offset(-2, -1, -2), center.offset(2, 3, 2), state);
    level.set_block(center, blocks::AIR.default_state(), UPDATE_ALL);
    level.set_block(center.above(), blocks::AIR.default_state(), UPDATE_ALL);
}

fn count(level: &mut ServerLevel, from: BlockPos, to: BlockPos, block: wafer_data::Block) -> usize {
    BlockPos::between_closed(from, to).filter(|&pos| level.block_state(pos).is_block(block)).count()
}

#[test]
fn silverfish_have_vanillas_attributes() {
    let mob = Silverfish::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 8.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.25);
    assert_eq!(attributes.value(Attribute::AttackDamage), 1.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 16.0);
    assert_eq!(mob.xp_reward, 5);
    assert_eq!((mob.entity.dimensions.width, mob.entity.dimensions.height, mob.entity.dimensions.eye_height), (0.4, 0.3, 0.13));
}

#[test]
fn infested_states_keep_their_properties() {
    let deepslate = blocks::DEEPSLATE.default_state().set(AXIS, wafer_core::Axis::X);
    let infested = infested_block::infested_state_by_host(deepslate);
    assert!(infested.is_block(blocks::INFESTED_DEEPSLATE));
    assert_eq!(infested.get(AXIS), wafer_core::Axis::X);
    assert_eq!(infested_block::host_state_by_infested(infested), deepslate);
    assert_eq!(infested_block::infested_state_by_host(blocks::STONE_BRICKS.default_state()), blocks::INFESTED_STONE_BRICKS.default_state());
    assert!(infested_block::is_compatible_host_block(blocks::COBBLESTONE.default_state()));
    assert!(!infested_block::is_compatible_host_block(blocks::INFESTED_STONE.default_state()));
    assert!(!infested_block::is_compatible_host_block(blocks::ANDESITE.default_state()));
    assert!(infested_block::is_infested_block(blocks::INFESTED_CHISELED_STONE_BRICKS.default_state()));
}

#[test]
fn breaking_infested_blocks_releases_silverfish() {
    let mut level = level();
    let pos = BlockPos::new(3, -60, 7);
    level.set_block(pos, blocks::INFESTED_STONE.default_state(), UPDATE_ALL);
    level.destroy_block(pos, true, None, 512);
    let spawned = silverfish(&level);
    assert_eq!(spawned.len(), 1);
    assert_eq!(level.entity(spawned[0]).position(), Vec3::new(3.5, -60.0, 7.5));
    level.game_rules.set(game_rule::BLOCK_DROPS, 0);
    level.set_block(pos, blocks::INFESTED_STONE.default_state(), UPDATE_ALL);
    level.destroy_block(pos, true, None, 512);
    assert_eq!(silverfish(&level).len(), 1);
}

#[test]
fn hurt_silverfish_wake_up_their_friends() {
    for mob_griefing in [true, false] {
        let mut level = level();
        level.game_rules.set(game_rule::MOB_GRIEFING, mob_griefing as i32);
        let center = BlockPos::new(0, -60, 0);
        pocket(&mut level, center, blocks::INFESTED_STONE.default_state());
        let player = level.players.keys().copied().next().expect("a player");
        let mut mob = Silverfish::create();
        mob.entity.random = LegacyRandomSource::new(7);
        let id = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
        let infested_before = count(&mut level, center.offset(-2, -1, -2), center.offset(2, 3, 2), blocks::INFESTED_STONE);
        assert!(level.hurt_server(id, DamageSource::source(damage_types::PLAYER_ATTACK, Some(player), Some(player)), 1.0));
        for _ in 0..30 {
            level.tick();
        }
        let infested_after = count(&mut level, center.offset(-2, -1, -2), center.offset(2, 3, 2), blocks::INFESTED_STONE);
        assert!(infested_after < infested_before);
        let woken = infested_before - infested_after;
        if mob_griefing {
            assert_eq!(silverfish(&level).len(), 1 + woken);
        } else {
            assert_eq!(silverfish(&level).len(), 1);
            assert_eq!(count(&mut level, center.offset(-2, -1, -2), center.offset(2, 3, 2), blocks::STONE), woken);
        }
    }
}

#[test]
fn silverfish_ignore_damage_without_a_cause() {
    let mut level = level();
    let center = BlockPos::new(0, -60, 0);
    pocket(&mut level, center, blocks::INFESTED_STONE.default_state());
    let id = add(&mut level, Silverfish::create(), Vec3::new(0.5, -60.0, 0.5));
    let infested_before = count(&mut level, center.offset(-2, -1, -2), center.offset(2, 3, 2), blocks::INFESTED_STONE);
    assert!(level.hurt_server(id, DamageSource::new(damage_types::GENERIC), 1.0));
    for _ in 0..30 {
        level.tick();
    }
    assert_eq!(count(&mut level, center.offset(-2, -1, -2), center.offset(2, 3, 2), blocks::INFESTED_STONE), infested_before);
    assert!(level.hurt_server(id, DamageSource::new(damage_types::MAGIC), 1.0));
    for _ in 0..30 {
        level.tick();
    }
    assert!(count(&mut level, center.offset(-2, -1, -2), center.offset(2, 3, 2), blocks::INFESTED_STONE) < infested_before);
}

#[test]
fn silverfish_hide_in_stone() {
    for mob_griefing in [true, false] {
        let mut level = level();
        level.game_rules.set(game_rule::MOB_GRIEFING, mob_griefing as i32);
        let center = BlockPos::new(0, -60, 0);
        pocket(&mut level, center, blocks::STONE.default_state());
        let mut mob = Silverfish::create();
        mob.entity.random = LegacyRandomSource::new(3);
        let id = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
        for _ in 0..200 {
            level.tick();
        }
        let infested = count(&mut level, center.offset(-1, -1, -1), center.offset(1, 2, 1), blocks::INFESTED_STONE);
        if mob_griefing {
            assert!(level.entities.get(&id).is_none());
            assert_eq!(infested, 1);
        } else {
            assert!(level.entities.get(&id).is_some());
            assert_eq!(infested, 0);
        }
    }
}

#[test]
fn silverfish_prefer_walking_on_host_blocks() {
    let mut level = level();
    let id = add(&mut level, Silverfish::create(), Vec3::new(0.5, -60.0, 0.5));
    level.set_block(BlockPos::new(5, -61, 5), blocks::STONE.default_state(), UPDATE_ALL);
    assert_eq!(level.get_walk_target_value(id, BlockPos::new(5, -60, 5)), 10.0);
    assert_eq!(level.get_walk_target_value(id, BlockPos::new(6, -60, 5)), level.monster_get_walk_target_value(BlockPos::new(6, -60, 5)));
}

#[test]
fn silverfish_bodies_follow_their_rotation() {
    let mut level = level();
    let id = add(&mut level, Silverfish::create(), Vec3::new(0.5, -60.0, 0.5));
    level.set_y_body_rot(id, 42.0);
    assert_eq!(level.entity(id).y_rot, 42.0);
    assert_eq!(level.living(id).y_body_rot, 42.0);
}

#[test]
fn the_infested_effect_releases_silverfish_when_hurt() {
    let mut level = level();
    let mut spawned = 0;
    for seed in 0..40 {
        let mut mob = crate::world::entity::monster::zombie::zombie::Zombie::create();
        mob.entity.random = LegacyRandomSource::new(seed);
        mob.persistence_required = true;
        let zombie = add(&mut level, mob, Vec3::new(seed as f64 * 2.0, -60.0, 10.5));
        level.add_effect(zombie, MobEffectInstance::new(mob_effects::INFESTED, -1, 0), None);
        let before = silverfish(&level);
        level.hurt_server(zombie, DamageSource::new(damage_types::GENERIC), 1.0);
        for id in silverfish(&level).into_iter().filter(|id| !before.contains(id)) {
            spawned += 1;
            let e = level.entity(id);
            assert_eq!(e.position(), Vec3::new(seed as f64 * 2.0, -60.0 + 1.95f32 as f64 / 2.0, 10.5));
            let motion = e.delta_movement;
            assert_eq!(motion.y, 0.0);
            assert!(((motion.x * motion.x + motion.z * motion.z).sqrt() - 0.3).abs() < 1.0e-6);
        }
    }
    assert!(spawned > 0);
}
