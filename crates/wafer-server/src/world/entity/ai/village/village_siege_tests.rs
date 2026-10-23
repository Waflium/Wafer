use wafer_core::{BlockPos, Vec3};

use super::poi::poi_types::PoiType;
use super::village_siege::VillageSiege;
use crate::server::level::{ServerLevel, testing};
use crate::world::clock::ServerClockManager;
use crate::world::difficulty::Difficulty;
use crate::world::entity::EntityId;

fn zombies(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.keys().copied().filter(|&id| wafer_data::registry::ENTITY_TYPE[level.entity(id).entity_type as usize] == "minecraft:zombie").collect()
}

fn village_level() -> ServerLevel {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    for x in -4..=4 {
        for z in -4..=4 {
            let pos = BlockPos::new(x * 16, -60, z * 16);
            level.chunks.poi_manager.add(pos, PoiType::Meeting);
            level.chunks.poi_manager.take(&|poi_type| poi_type == PoiType::Meeting, |_, found| found == pos, pos, 1);
        }
    }
    level
}

fn set_time(level: &mut ServerLevel, ticks: i64) {
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), ticks);
    level.sync_clocks(&clocks);
    level.update_sky_brightness();
}

#[test]
fn sieges_send_twenty_zombies_into_villages_at_night() {
    let mut level = village_level();
    let mut siege = VillageSiege::default();
    let mut rolls = 0;
    while zombies(&level).is_empty() && rolls < 1000 {
        set_time(&mut level, 18000);
        siege.tick(&mut level, true);
        set_time(&mut level, 18001);
        for _ in 0..4 {
            siege.tick(&mut level, true);
        }
        rolls += 1;
    }
    assert!(!zombies(&level).is_empty());
    for _ in 0..100 {
        siege.tick(&mut level, true);
    }
    let spawned = zombies(&level);
    assert!(spawned.len() <= 20 && spawned.len() > 1);
    for &zombie in &spawned {
        let pos = level.entity(zombie).position();
        assert_eq!(pos.y, -60.0);
        assert_eq!(pos.x - pos.x.floor(), 0.5);
        assert!(level.is_village(level.entity(zombie).block_position()));
        assert!(pos.x.abs().max(pos.z.abs()) >= 32.0 * std::f64::consts::FRAC_1_SQRT_2 - 9.0);
    }
    let count = spawned.len();
    for _ in 0..100 {
        siege.tick(&mut level, true);
    }
    assert_eq!(zombies(&level).len(), count);
}

#[test]
fn sieges_stop_in_daylight_and_outside_villages() {
    let mut level = village_level();
    let mut siege = VillageSiege::default();
    for _ in 0..200 {
        set_time(&mut level, 18000);
        siege.tick(&mut level, false);
        set_time(&mut level, 6000);
        siege.tick(&mut level, true);
    }
    assert!(zombies(&level).is_empty());
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    for _ in 0..200 {
        set_time(&mut level, 18000);
        for _ in 0..50 {
            siege.tick(&mut level, true);
        }
    }
    assert!(zombies(&level).is_empty());
}


#[test]
fn village_distance_spreads_from_occupied_village_pois() {
    let mut level = testing::level();
    let pos = BlockPos::new(8, -60, 8);
    let center = BlockPos::new(0, -4, 0);
    assert_eq!(level.sections_to_village(center), 7);
    level.chunks.poi_manager.add(pos, PoiType::Home);
    assert_eq!(level.sections_to_village(center), 7);
    level.chunks.poi_manager.take(&|poi_type| poi_type == PoiType::Home, |_, found| found == pos, pos, 1);
    assert_eq!(level.sections_to_village(center), 0);
    assert_eq!(level.sections_to_village(BlockPos::new(2, -3, -2)), 2);
    assert_eq!(level.sections_to_village(BlockPos::new(-5, -4, 5)), 5);
    assert_eq!(level.sections_to_village(BlockPos::new(-6, -4, 6)), 7);
    assert_eq!(level.sections_to_village(BlockPos::new(7, -4, 0)), 7);
    level.chunks.poi_manager.remove(pos);
    assert_eq!(level.sections_to_village(BlockPos::new(2, -3, -2)), 6);
}
