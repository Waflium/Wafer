use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, items};

use super::patrol_spawner::PatrolSpawner;
use crate::server::level::{ServerLevel, testing};
use crate::world::clock::ServerClockManager;
use crate::world::difficulty::Difficulty;
use crate::world::entity::EntityId;

fn late_game_level() -> ServerLevel {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), 126000);
    level.sync_clocks(&clocks);
    level.update_sky_brightness();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level
}

fn pillagers(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.keys().copied().filter(|&id| wafer_data::registry::ENTITY_TYPE[level.entity(id).entity_type as usize] == "minecraft:pillager").collect()
}

#[test]
fn patrols_spawn_a_leader_and_followers_away_from_the_player() {
    let mut level = late_game_level();
    let mut spawner = PatrolSpawner::default();
    let mut found = Vec::new();
    for _ in 0..2_000_000 {
        spawner.tick(&mut level, true);
        found = pillagers(&level);
        if !found.is_empty() {
            break;
        }
    }
    assert!(!found.is_empty());
    let leader = found[0];
    let mob = level.mob(leader).unwrap();
    let patrolling_monster = mob.patrolling_monster().unwrap();
    assert!(patrolling_monster.is_patrol_leader());
    assert!(patrolling_monster.is_patrolling());
    assert!(patrolling_monster.has_patrol_target());
    assert_eq!(mob.equipment[EquipmentSlot::Head as usize].as_ref().map(|stack| stack.item), Some(items::WHITE_BANNER));
    assert_eq!(mob.equipment[EquipmentSlot::Mainhand as usize].as_ref().map(|stack| stack.item), Some(items::CROSSBOW));
    let pos = level.entity(leader).position();
    assert_eq!(pos.y, -60.0);
    assert_eq!(pos.x, pos.x.floor());
    assert_eq!(pos.z, pos.z.floor());
    assert!(pos.x.abs() >= 23.0 && pos.z.abs() >= 23.0);
    for &follower in &found[1..] {
        let patrolling_monster = level.mob(follower).unwrap().patrolling_monster().unwrap();
        assert!(!patrolling_monster.is_patrol_leader());
        assert!(patrolling_monster.is_patrolling());
        assert!(!patrolling_monster.has_patrol_target());
    }
    assert!(found.len() <= 4);
}

#[test]
fn patrols_need_enemies_and_the_game_rule() {
    let mut level = late_game_level();
    let mut spawner = PatrolSpawner::default();
    for _ in 0..200_000 {
        spawner.tick(&mut level, false);
    }
    assert!(pillagers(&level).is_empty());
    level.game_rules.set(wafer_data::game_rule::SPAWN_PATROLS, 0);
    for _ in 0..200_000 {
        spawner.tick(&mut level, true);
    }
    assert!(pillagers(&level).is_empty());
}


#[test]
fn patrols_wait_for_the_early_game_to_end() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut spawner = PatrolSpawner::default();
    for _ in 0..200_000 {
        spawner.tick(&mut level, true);
    }
    assert!(pillagers(&level).is_empty());
}
