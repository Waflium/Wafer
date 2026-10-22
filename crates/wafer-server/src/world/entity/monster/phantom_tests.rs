use wafer_core::{BlockPos, Vec3};
use wafer_protocol::nbt::Nbt;

use super::phantom::{self, Phantom};
use crate::server::level::{ServerLevel, testing};
use crate::server::level::game_mode::GameType;
use crate::world::clock::ServerClockManager;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::levelgen::phantom_spawner::PhantomSpawner;
use crate::stats::stats;
use crate::stats::stats_counter::StatsCounter;

fn phantoms(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.keys().copied().filter(|&id| level.is_phantom(id)).collect()
}

fn night_level() -> ServerLevel {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), 18000);
    level.sync_clocks(&clocks);
    level.update_sky_brightness();
    level
}

#[test]
fn phantoms_have_vanillas_attributes_and_goals() {
    let mob = Phantom::create();
    assert_eq!(mob.living.attributes.value(Attribute::MaxHealth), 20.0);
    assert_eq!(mob.living.attributes.value(Attribute::AttackDamage), 2.0);
    assert_eq!(mob.xp_reward, 5);
    let priorities: Vec<i32> = mob.ai.goal_selector.get_available_goals().iter().map(|wrapped| wrapped.get_priority()).collect();
    assert_eq!(priorities, vec![1, 2, 3]);
    assert!(matches!(mob.ai.target_selector.get_available_goals()[0].get_goal(), Goal::PhantomAttackPlayerTarget(_)));
    assert_eq!(mob.client_tracking_range(), 8);
}

#[test]
fn phantom_size_scales_dimensions_and_attack_damage() {
    let mut mob = Phantom::create();
    Phantom::set_phantom_size(&mut mob, 4);
    assert_eq!(phantom::get_phantom_size(&mob.entity), 4);
    assert_eq!(mob.entity.dimensions.width, 0.9 * (1.0 + 0.15 * 4.0));
    assert_eq!(mob.living.attributes.value(Attribute::AttackDamage), 10.0);
    Phantom::set_phantom_size(&mut mob, 100);
    assert_eq!(phantom::get_phantom_size(&mob.entity), 64);
    let mut out = Vec::new();
    Phantom::add_additional_save_data(&mob, &mut out);
    assert_eq!(out, vec![("size".to_string(), Nbt::Int(64))]);
}

#[test]
fn finalized_phantoms_anchor_five_blocks_up() {
    let mut level = night_level();
    let mut mob = Phantom::create();
    mob.entity.snap_to(3.5, -40.0, 7.5, 0.0, 0.0);
    let difficulty = level.current_difficulty_at(BlockPos::new(3, -40, 7));
    let (mob, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, EntitySpawnReason::Natural, None);
    let LevelEntity::Mob(mob) = mob else { unreachable!() };
    let mut out = Vec::new();
    Phantom::add_additional_save_data(&mob, &mut out);
    assert_eq!(out[0], ("anchor_pos".to_string(), Nbt::IntArray(vec![3, -35, 7])));
}

#[test]
fn phantoms_swoop_at_and_bite_survival_players() {
    let mut level = night_level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).movement.client_loaded_timeout_timer = 0;
    level.player_mut(player).game_mode.game_type = GameType::Survival;
    let mut mob = Phantom::create();
    mob.entity.snap_to(0.5, -40.0, -6.5, 0.0, 0.0);
    let id = mob.entity.id;
    let difficulty = level.current_difficulty_at(BlockPos::new(0, -40, -6));
    let (mob, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, EntitySpawnReason::Natural, None);
    level.add_fresh_entity(mob);
    let mut targeted = false;
    for _ in 0..1200 {
        level.tick();
        targeted |= level.get_target(id) == Some(player);
        if level.health(player) < 20.0 {
            break;
        }
    }
    assert!(targeted);
    assert!(level.health(player) < 20.0);
}

#[test]
fn phantoms_spawn_above_sleepless_players_at_night() {
    let mut level = night_level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).game_mode.game_type = GameType::Survival;
    let mut spawner = PhantomSpawner::default();
    for _ in 0..200_000 {
        spawner.tick(&mut level, true);
    }
    assert!(phantoms(&level).is_empty());
    level.player_mut(player).stats.set_value(stats::TIME_SINCE_REST, 1_000_000);
    let mut found = Vec::new();
    for _ in 0..2_000_000 {
        spawner.tick(&mut level, true);
        found = phantoms(&level);
        if !found.is_empty() {
            break;
        }
    }
    assert!(!found.is_empty() && found.len() <= 3);
    let pos = level.entity(found[0]).position();
    assert!((-40.0..=-26.0).contains(&pos.y));
    assert!(pos.x.abs() <= 11.0 && pos.z.abs() <= 11.0);
    assert_eq!(pos.x - pos.x.floor(), 0.5);
    for &phantom in &found {
        assert_eq!(level.entity(phantom).position(), pos);
    }
}

#[test]
fn phantoms_need_darkness() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).stats.set_value(stats::TIME_SINCE_REST, 1_000_000);
    let mut spawner = PhantomSpawner::default();
    for _ in 0..200_000 {
        spawner.tick(&mut level, true);
    }
    assert!(phantoms(&level).is_empty());
}
