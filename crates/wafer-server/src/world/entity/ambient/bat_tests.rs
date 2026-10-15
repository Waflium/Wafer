use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, damage_types};
use wafer_protocol::nbt::Nbt;

use super::bat::Bat;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::update_flags::UPDATE_ALL;

#[derive(Debug, PartialEq)]
struct Row {
    x: f64,
    y: f64,
    z: f64,
    y_rot: f32,
    health: f32,
    resting: bool,
}

fn oracle_rows(label: &str) -> Vec<Row> {
    include_str!("bat_oracle.txt")
        .lines()
        .skip_while(|line| *line != format!("== {label}"))
        .skip(1)
        .take_while(|line| !line.starts_with("=="))
        .map(|line| {
            let parts: Vec<&str> = line.split(' ').collect();
            let field = |key: &str| parts.iter().find_map(|part| part.strip_prefix(key)).unwrap();
            Row {
                x: parts[0].parse().unwrap(),
                y: parts[1].parse().unwrap(),
                z: parts[2].parse().unwrap(),
                y_rot: parts[3].parse().unwrap(),
                health: field("health=").parse().unwrap(),
                resting: field("resting=").parse().unwrap(),
            }
        })
        .collect()
}

fn build_ceiling(level: &mut ServerLevel) {
    for x in -6..=6 {
        for z in -40..=-28 {
            level.set_block(BlockPos::new(x, -58, z), blocks::STONE.default_state(), UPDATE_ALL);
        }
    }
    for x in -6..=6 {
        for y in -64..=-61 {
            for z in -60..=-48 {
                let inside = x > -6 && x < 6 && y > -64 && y < -61 && z > -60 && z < -48;
                let state = if inside { blocks::AIR.default_state() } else { blocks::STONE.default_state() };
                level.set_block(BlockPos::new(x, y, z), state, UPDATE_ALL);
            }
        }
    }
}

struct Scenario {
    start: Vec3,
    resting: bool,
    seed: i64,
    parity: i32,
    hurt_after: Option<usize>,
    player_moves: Option<(usize, Vec3)>,
}

fn play(scenario: &Scenario, ticks: usize) -> Vec<Row> {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    build_ceiling(&mut level);
    for _ in 0..61 {
        level.tick();
    }
    let mut mob = loop {
        let mob = Bat::create();
        if mob.entity.id.rem_euclid(2) == scenario.parity {
            break mob;
        }
    };
    mob.persistence_required = true;
    mob.set_resting(scenario.resting);
    mob.entity.snap_to(scenario.start.x, scenario.start.y, scenario.start.z, 0.0, 0.0);
    mob.entity.y_head_rot = 0.0;
    mob.living.y_body_rot = 0.0;
    mob.entity.random = LegacyRandomSource::new(scenario.seed);
    let id: EntityId = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    let mut rows = Vec::new();
    for tick in 0..ticks {
        if scenario.hurt_after == Some(tick) {
            assert!(level.hurt_server(id, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 1.0));
        }
        if let Some((after, pos)) = scenario.player_moves
            && after == tick
        {
            level.entity_mut(player).set_pos(pos);
        }
        level.tick();
        let mob = level.mob(id).unwrap();
        let e = &mob.entity;
        rows.push(Row { x: e.x(), y: e.y(), z: e.z(), y_rot: e.y_rot, health: mob.health, resting: mob.is_resting() });
    }
    rows
}

fn assert_matches_oracle(label: &str, scenario: Scenario) {
    let expected = oracle_rows(label);
    assert!(!expected.is_empty(), "no oracle rows for {label}");
    let actual = play(&scenario, expected.len());
    for (tick, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "{label} tick {}", tick + 1);
    }
}

#[test]
fn bats_fly_like_vanilla() {
    assert_matches_oracle("bat_fly", Scenario { start: Vec3::new(0.5, -55.0, -15.5), resting: false, seed: 42, parity: 0, hurt_after: None, player_moves: None });
}

#[test]
fn resting_bats_wake_up_near_players_like_vanilla() {
    assert_matches_oracle(
        "bat_rest",
        Scenario { start: Vec3::new(0.5, -59.0, -34.5), resting: true, seed: 7, parity: 1, hurt_after: None, player_moves: Some((40, Vec3::new(0.5, -60.0, -31.5))) },
    );
}

#[test]
fn hurt_bats_wake_up_like_vanilla() {
    assert_matches_oracle("bat_hurt", Scenario { start: Vec3::new(0.5, -59.0, -34.5), resting: true, seed: 9, parity: 0, hurt_after: Some(5), player_moves: None });
}

#[test]
fn bats_hang_from_ceilings_like_vanilla() {
    assert_matches_oracle("bat_roost", Scenario { start: Vec3::new(0.5, -63.0, -54.5), resting: false, seed: 21, parity: 1, hurt_after: None, player_moves: None });
}

#[test]
fn bats_rest_when_spawned_and_fly_when_loaded_without_flags() {
    let mut bat = Bat::create();
    assert!(bat.is_resting());
    let mut out = Vec::new();
    bat.save(&mut out);
    assert_eq!(out.iter().find(|(k, _)| k == "BatFlags").map(|(_, v)| v.clone()), Some(Nbt::Byte(1)));
    bat.load(&Nbt::Compound(Vec::new()));
    assert!(!bat.is_resting());
}
