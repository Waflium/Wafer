use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, damage_types};
use wafer_protocol::nbt::Nbt;

use super::glow_squid::GlowSquid;
use super::squid::{Squid, squid_mut};
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::mob::Mob;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::update_flags::UPDATE_ALL;

#[derive(Debug, PartialEq)]
struct Row {
    x: f64,
    y: f64,
    z: f64,
    y_rot: f32,
    air: i32,
    health: f32,
    dark: Option<i32>,
}

fn oracle_rows(label: &str) -> Vec<Row> {
    include_str!("squid_oracle.txt")
        .lines()
        .skip_while(|line| *line != format!("== {label}"))
        .skip(1)
        .take_while(|line| !line.starts_with("=="))
        .map(|line| {
            let parts: Vec<&str> = line.split(' ').collect();
            let field = |key: &str| parts.iter().find_map(|part| part.strip_prefix(key));
            Row {
                x: parts[0].parse().unwrap(),
                y: parts[1].parse().unwrap(),
                z: parts[2].parse().unwrap(),
                y_rot: parts[3].parse().unwrap(),
                air: field("air=").unwrap().parse().unwrap(),
                health: field("health=").unwrap().parse().unwrap(),
                dark: field("dark=").map(|dark| dark.parse().unwrap()),
            }
        })
        .collect()
}

fn build_pool(level: &mut ServerLevel) {
    for x in -9..=9 {
        for y in -63..=-55 {
            for z in 9..=27 {
                let inside = (-8..=8).contains(&x) && (10..=26).contains(&z);
                let state = if inside { blocks::WATER.default_state() } else { blocks::GLASS.default_state() };
                level.set_block(BlockPos::new(x, y, z), state, UPDATE_ALL);
            }
        }
    }
}

struct Scenario {
    create: fn() -> Mob,
    start: Vec3,
    player: Vec3,
    seed: i64,
    parity: i32,
    baby: bool,
    hurt_after: Option<usize>,
}

fn row(level: &ServerLevel, id: EntityId) -> Row {
    let mob = level.mob(id).unwrap();
    let e = &mob.entity;
    let dark = matches!(mob.kind, crate::world::entity::mob::MobKind::GlowSquid(_)).then(|| mob.get_dark_ticks_remaining());
    Row { x: e.x(), y: e.y(), z: e.z(), y_rot: e.y_rot, air: e.air_supply(), health: mob.health, dark }
}

fn play(scenario: &Scenario, ticks: usize) -> Vec<Row> {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, scenario.player, 0.0, 0.0);
    build_pool(&mut level);
    for _ in 0..61 {
        level.tick();
    }
    let mut mob = loop {
        let mob = (scenario.create)();
        if mob.entity.id.rem_euclid(2) == scenario.parity {
            break mob;
        }
    };
    mob.persistence_required = true;
    if scenario.baby {
        mob.set_baby(true);
    }
    mob.entity.snap_to(scenario.start.x, scenario.start.y, scenario.start.z, 0.0, 0.0);
    mob.entity.y_head_rot = 0.0;
    mob.living.y_body_rot = 0.0;
    mob.entity.random = LegacyRandomSource::new(scenario.seed);
    squid_mut(&mut mob).tentacle_speed = 0.15;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    let mut rows = Vec::new();
    for tick in 0..ticks {
        if scenario.hurt_after == Some(tick) {
            assert!(level.hurt_server(id, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 1.0));
        }
        level.tick();
        rows.push(row(&level, id));
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

const POOL_PLAYER: Vec3 = Vec3::new(0.5, -60.0, 7.5);

#[test]
fn squids_swim_like_vanilla() {
    assert_matches_oracle(
        "squid_swim",
        Scenario { create: Squid::create, start: Vec3::new(0.5, -59.0, 18.5), player: Vec3::new(0.5, -60.0, 0.5), seed: 42, parity: 1, baby: false, hurt_after: None },
    );
}

#[test]
fn hurt_squids_ink_and_flee_like_vanilla() {
    assert_matches_oracle(
        "squid_flee",
        Scenario { create: Squid::create, start: Vec3::new(0.5, -59.0, 13.5), player: POOL_PLAYER, seed: 5, parity: 0, baby: false, hurt_after: Some(20) },
    );
}

#[test]
fn hurt_glow_squids_go_dark_like_vanilla() {
    assert_matches_oracle(
        "glow",
        Scenario { create: GlowSquid::create, start: Vec3::new(0.5, -59.0, 13.5), player: POOL_PLAYER, seed: 11, parity: 1, baby: false, hurt_after: Some(10) },
    );
}

#[test]
fn baby_squids_ink_like_vanilla() {
    assert_matches_oracle(
        "baby",
        Scenario { create: Squid::create, start: Vec3::new(0.5, -59.0, 13.5), player: POOL_PLAYER, seed: 13, parity: 0, baby: true, hurt_after: Some(30) },
    );
}

#[test]
fn squids_on_land_suffocate_like_vanilla() {
    assert_matches_oracle(
        "squid_land",
        Scenario { create: Squid::create, start: Vec3::new(0.5, -60.0, -5.5), player: POOL_PLAYER, seed: 3, parity: 1, baby: false, hurt_after: None },
    );
}

#[test]
fn squids_save_and_load_like_vanilla() {
    let mut glow_squid = GlowSquid::create();
    glow_squid.set_baby(true);
    let mut out = Vec::new();
    glow_squid.save(&mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("DarkTicksRemaining"), Some(Nbt::Int(0)));
    assert_eq!(get("Age"), Some(Nbt::Int(-24000)));
    let mut loaded = GlowSquid::create();
    loaded.load(&Nbt::Compound(vec![("DarkTicksRemaining".into(), Nbt::Int(42)), ("Age".into(), Nbt::Int(-5))].into()));
    assert_eq!(loaded.get_dark_ticks_remaining(), 42);
    assert!(loaded.is_baby());
    assert_eq!(loaded.entity.dimensions.width, 0.5);
}
