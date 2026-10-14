use wafer_core::random::LegacyRandomSource;
use wafer_core::{ChunkPos, Vec3};
use wafer_data::game_rule;

use crate::server::Server;
use crate::server::level::testing;
use crate::world::entity::LevelEntity;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::mob_category::MobCategory;

fn server() -> Server {
    let mut server = testing::server();
    server.set_game_rule(game_rule::SPAWN_MOBS, 1);
    server
}

fn tick_until(server: &mut Server, game_time: i64) {
    while server.overworld().game_time < game_time {
        server.tick();
    }
}

fn mob_positions(server: &Server) -> Vec<(&'static str, Vec3)> {
    server.overworld().entities.values().filter_map(|e| match e {
        LevelEntity::Mob(mob) => Some((EntityType::by_id(mob.entity.entity_type).get_key(), mob.entity.position())),
        _ => None,
    })
    .collect()
}

fn is_close_enough_for_spawning(mob: Vec3, player: Vec3) -> bool {
    let chunk = ChunkPos::new(mob.x.floor() as i32 >> 4, mob.z.floor() as i32 >> 4);
    let (dx, dz) = ((chunk.min_block_x() + 8) as f64 - player.x, (chunk.min_block_z() + 8) as f64 - player.z);
    dx * dx + dz * dz < 16384.0
}

fn joined(seed: i64) -> Server {
    let mut server = server();
    server.set_game_rule(game_rule::SPAWN_MONSTERS, 0);
    testing::server_player(&mut server, Vec3::new(8.5, -60.0, 8.5), 0.0, 0.0);
    server.overworld_mut().chunks.run_until_idle();
    server.overworld_mut().process_chunk_events();
    server.overworld_mut().random = LegacyRandomSource::new(seed);
    server
}

#[test]
fn creatures_spawn_every_400_ticks_away_from_the_player() {
    let mut server = joined(1);
    tick_until(&mut server, 399);
    assert!(mob_positions(&server).is_empty(), "{:?} at {}", mob_positions(&server), server.overworld().game_time);
    tick_until(&mut server, 400);
    let mobs = mob_positions(&server);
    let counted_by_the_local_cap = mobs.iter().filter(|(_, pos)| is_close_enough_for_spawning(*pos, Vec3::new(8.5, -60.0, 8.5))).count();
    assert!(!mobs.is_empty() && counted_by_the_local_cap <= 13, "{mobs:?}");
    for (name, pos) in &mobs {
        assert_eq!(EntityType::by_name(name).unwrap().get_category(), MobCategory::Creature);
        assert!(["minecraft:chicken", "minecraft:cow", "minecraft:pig", "minecraft:sheep"].contains(name), "{name}");
        let (dx, dz) = (pos.x - 8.5, pos.z - 8.5);
        assert!(dx * dx + (pos.y + 60.0) * (pos.y + 60.0) + dz * dz > 24.0 * 24.0, "{name} at {pos:?}");
        assert_eq!(pos.y, -60.0);
    }
}

#[test]
fn the_spawn_mobs_rule_stops_natural_spawning() {
    let mut server = joined(1);
    server.set_game_rule(game_rule::SPAWN_MOBS, 0);
    tick_until(&mut server, 800);
    assert!(mob_positions(&server).is_empty());
}

#[test]
fn creatures_count_towards_the_local_cap() {
    let mut server = joined(2);
    tick_until(&mut server, 400);
    let first = mob_positions(&server).len();
    tick_until(&mut server, 800);
    let state = server.overworld().chunks.last_spawn_state.as_ref().unwrap();
    assert_eq!(state.get_mob_category_counts()[MobCategory::Creature as usize] as usize, mob_positions(&server).len());
    assert!(first >= 10 || mob_positions(&server).len() > first);
}
