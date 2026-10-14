use wafer_core::Vec3;
use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_data::{BlockState, blocks, damage_types, items};
use wafer_protocol::nbt::Nbt;

use super::magma_cube::MagmaCube;
use super::slime::Slime;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::Mob;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::levelgen::worldgen_random::WorldgenRandom;
use crate::world::level::update_flags::UPDATE_ALL;
use wafer_core::BlockPos;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn sized(mut mob: Mob, size: i32) -> Mob {
    mob.set_size(size, true);
    mob
}

fn get(out: &[(String, Nbt)], key: &str) -> Option<Nbt> {
    out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
}

fn loaded_level(player_pos: Vec3) -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, player_pos, 0.0, 0.0);
    for _ in 0..61 {
        level.tick();
    }
    (level, player)
}

fn cube_mobs(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.iter().filter(|(_, e)| matches!(e, LevelEntity::Mob(mob) if mob.is_cube_mob() && !mob.entity.is_removed())).map(|(id, _)| *id).collect()
}

struct Scenario {
    create: fn() -> Mob,
    size: i32,
    start: Vec3,
    seed: i64,
    parity: i32,
    target: bool,
    pool: Option<(BlockState, i32)>,
}

const NO_POOL: Option<(BlockState, i32)> = None;

fn fill_pool(level: &mut ServerLevel, state: BlockState, z0: i32) {
    for x in -5..=5 {
        for y in -63..=-61 {
            for z in z0..=z0 + 10 {
                level.set_block(BlockPos::new(x, y, z), state, UPDATE_ALL);
            }
        }
    }
}

fn oracle_rows(text: &str, label: &str) -> Vec<(f64, f64, f64)> {
    text.lines()
        .skip_while(|line| *line != format!("== {label}"))
        .skip(1)
        .take_while(|line| !line.starts_with("=="))
        .map(|line| {
            let v: Vec<f64> = line.split(' ').map(|n| n.parse().unwrap()).collect();
            (v[0], v[1], v[2])
        })
        .collect()
}

fn play(scenario: &Scenario, ticks: usize) -> Vec<(f64, f64, f64)> {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut scoreboard = level.scoreboard.borrow_mut();
    scoreboard.add_player_team("t");
    scoreboard.set_collision_rule("t", crate::world::scores::team::CollisionRule::Never);
    scoreboard.add_player_to_team_base("Tester", "t");
    drop(scoreboard);
    if let Some((state, z0)) = scenario.pool {
        fill_pool(&mut level, state, z0);
    }
    for _ in 0..61 {
        level.tick();
    }
    let mob = loop {
        let mob = (scenario.create)();
        if mob.entity.id.rem_euclid(2) == scenario.parity {
            break mob;
        }
    };
    let mut mob = sized(mob, scenario.size);
    mob.persistence_required = true;
    mob.entity.y_head_rot = 0.0;
    mob.living.y_body_rot = 0.0;
    mob.abstract_cube_mob_mut().unwrap().move_control.y_rot = 0.0;
    let id = add(&mut level, mob, scenario.start);
    level.mob_mut(id).unwrap().entity.random = LegacyRandomSource::new(scenario.seed);
    if scenario.target {
        level.set_target(id, Some(player));
    }
    let mut rows = Vec::new();
    for _ in 0..ticks {
        level.tick();
        let e = level.entity(id);
        rows.push((e.x(), e.y(), e.z()));
    }
    rows
}

fn assert_matches_oracle(label: &str, scenario: Scenario) {
    let expected = oracle_rows(include_str!("cube_mob_oracle.txt"), label);
    assert!(!expected.is_empty(), "no oracle rows for {label}");
    let actual = play(&scenario, expected.len());
    for (tick, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "{label} tick {}", tick + 1);
    }
}

#[test]
fn tiny_slimes_hop_toward_the_player_like_vanilla() {
    assert_matches_oracle("slime1", Scenario { create: Slime::create, size: 1, start: Vec3::new(8.5, -60.0, 0.5), seed: 42, parity: 0, target: true, pool: NO_POOL });
}

#[test]
fn big_slimes_hop_toward_the_player_like_vanilla() {
    assert_matches_oracle("slime4", Scenario { create: Slime::create, size: 4, start: Vec3::new(12.5, -60.0, 0.5), seed: 7, parity: 1, target: true, pool: NO_POOL });
}

#[test]
fn magma_cubes_hop_toward_the_player_like_vanilla() {
    assert_matches_oracle("magma2", Scenario { create: MagmaCube::create, size: 2, start: Vec3::new(10.5, -60.0, 0.5), seed: 5, parity: 0, target: true, pool: NO_POOL });
}

#[test]
fn slimes_without_a_target_wander_like_vanilla() {
    assert_matches_oracle("wander2", Scenario { create: Slime::create, size: 2, start: Vec3::new(0.5, -60.0, 40.5), seed: 11, parity: 0, target: false, pool: NO_POOL });
}

#[test]
fn slimes_float_in_water_like_vanilla() {
    let pool = Some((blocks::WATER.default_state(), 60));
    assert_matches_oracle("water2", Scenario { create: Slime::create, size: 2, start: Vec3::new(0.5, -61.0, 65.5), seed: 13, parity: 1, target: false, pool });
}

#[test]
fn magma_cubes_swim_in_lava_like_vanilla() {
    let pool = Some((blocks::LAVA.default_state(), 80));
    assert_matches_oracle("lava2", Scenario { create: MagmaCube::create, size: 2, start: Vec3::new(0.5, -61.0, 85.5), seed: 17, parity: 0, target: false, pool });
}

#[test]
fn killed_slimes_split_like_vanilla() {
    let (mut level, _) = loaded_level(Vec3::new(0.5, -60.0, 0.5));
    let mob = loop {
        let mob = Slime::create();
        if mob.entity.id.rem_euclid(2) == 1 {
            break mob;
        }
    };
    let mut mob = sized(mob, 4);
    mob.persistence_required = true;
    mob.entity.y_head_rot = 0.0;
    mob.living.y_body_rot = 0.0;
    mob.abstract_cube_mob_mut().unwrap().move_control.y_rot = 0.0;
    mob.entity.apply_implicit_component(crate::world::item::component::CUSTOM_NAME, &Nbt::String("Bob".into()));
    let parent = add(&mut level, mob, Vec3::new(20.5, -60.0, 0.5));
    level.mob_mut(parent).unwrap().entity.random = LegacyRandomSource::new(3);
    for _ in 0..5 {
        level.tick();
    }
    level.kill(parent);
    let mut ticks = 0;
    while level.entity_ref(parent).is_some() {
        level.tick();
        ticks += 1;
    }
    assert_eq!(ticks, 20);
    let mut children: Vec<(f64, f64, f64, f32, i32)> = cube_mobs(&level)
        .into_iter()
        .map(|id| {
            let mob = level.mob(id).unwrap();
            assert_eq!(mob.health, 4.0);
            assert!(mob.persistence_required);
            assert_eq!(mob.entity.custom_name().map(|name| name.get_string()), Some("Bob".to_string()));
            (mob.entity.x(), mob.entity.y(), mob.entity.z(), mob.entity.y_rot, mob.get_size())
        })
        .collect();
    children.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let mut expected = vec![
        (18.058976995422615, -59.5, -0.02961910885602531, 273.3905f32, 2),
        (19.098976957275642, -59.5, -0.02961910885602531, 162.72621, 2),
        (18.058976995422615, -59.5, 1.010380852997002, 320.9472, 2),
        (19.098976957275642, -59.5, 1.010380852997002, 109.533394, 2),
    ];
    expected.sort_by(|a, b| a.partial_cmp(b).unwrap());
    assert_eq!(children, expected);
}

#[test]
fn slime_chunks_match_vanilla() {
    for line in include_str!("slime_chunk_oracle.txt").lines() {
        let (seed, expected) = line.split_once(' ').unwrap();
        let seed: i64 = seed.parse().unwrap();
        let mut actual = String::new();
        for z in -20..=20 {
            for x in -20..=20 {
                let slime_chunk = WorldgenRandom::seed_slime_chunk(x * 977, z * 1301, seed, 987234911).next_int_bounded(10) == 0;
                actual.push(if slime_chunk { '1' } else { '0' });
            }
        }
        assert_eq!(actual, expected, "seed {seed}");
    }
}

#[test]
fn size_sets_attributes_dimensions_and_experience() {
    for size in [1, 2, 4] {
        let mob = sized(Slime::create(), size);
        let attributes = &mob.living.attributes;
        assert_eq!(attributes.value(Attribute::MaxHealth), (size * size) as f64);
        assert_eq!(attributes.value(Attribute::MovementSpeed), (0.2f32 + 0.1f32 * size as f32) as f64);
        assert_eq!(attributes.value(Attribute::AttackDamage), size as f64);
        assert_eq!(mob.health, (size * size) as f32);
        assert_eq!(mob.xp_reward, size);
        assert_eq!(mob.entity.dimensions.width, 0.52 * size as f32);
        assert_eq!(mob.entity.dimensions.eye_height, 0.325 * size as f32);
        let mob = sized(MagmaCube::create(), size);
        let attributes = &mob.living.attributes;
        assert_eq!(attributes.value(Attribute::Armor), (size * 3) as f64);
        assert_eq!(attributes.value(Attribute::AttackDamage), size as f64);
        assert_eq!(mob.xp_reward, size);
    }
    assert_eq!(sized(Slime::create(), 300).get_size(), 127);
    assert_eq!(sized(Slime::create(), -3).get_size(), 1);
}

#[test]
fn size_and_ground_state_save_and_load() {
    let mut mob = sized(Slime::create(), 4);
    mob.abstract_cube_mob_mut().unwrap().was_on_ground = true;
    let mut out = Vec::new();
    mob.save(&mut out);
    assert_eq!(get(&out, "Size"), Some(Nbt::Int(3)));
    assert_eq!(get(&out, "wasOnGround"), Some(Nbt::Byte(1)));
    assert_eq!(get(&out, "Age"), None);
    assert_eq!(get(&out, "AgeLocked"), None);
    let mut loaded = Slime::create();
    loaded.load(&Nbt::Compound(out));
    assert_eq!(loaded.get_size(), 4);
    assert!(loaded.abstract_cube_mob().unwrap().was_on_ground);
    assert_eq!(loaded.living.attributes.value(Attribute::MaxHealth), 16.0);
    let mut defaulted = Slime::create();
    defaulted.load(&Nbt::Compound(vec![("Age".into(), Nbt::Int(-24000))]));
    assert_eq!(defaulted.get_size(), 1);
    assert!(!defaulted.is_baby());
}

#[test]
fn natural_sizes_are_one_two_or_four() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Hard;
    let mut seen = [false; 5];
    for _ in 0..64 {
        let id = level.spawn_entity_type("minecraft:slime", BlockPos::new(0, -60, 0), crate::world::entity::entity_spawn_reason::EntitySpawnReason::Natural).unwrap();
        let mob = level.mob(id).unwrap();
        let size = mob.get_size();
        assert!(matches!(size, 1 | 2 | 4), "size {size}");
        assert_eq!(mob.health, (size * size) as f32);
        seen[size as usize] = true;
    }
    assert!(seen[1] && seen[2] && seen[4]);
}

#[test]
fn tiny_slimes_do_not_split() {
    let (mut level, _) = loaded_level(Vec3::new(0.5, -60.0, 30.5));
    let parent = add(&mut level, sized(Slime::create(), 1), Vec3::new(0.5, -60.0, 0.5));
    level.hurt_server(parent, DamageSource::new(damage_types::GENERIC_KILL), 100.0);
    for _ in 0..25 {
        level.tick();
    }
    assert!(cube_mobs(&level).is_empty());
}

fn touch_damage(mob: Mob) -> f32 {
    let (mut level, player) = loaded_level(Vec3::new(0.5, -60.0, 0.5));
    let id = add(&mut level, mob, Vec3::new(0.5, -60.0, 1.0));
    let before = level.health(player);
    level.abstract_cube_mob_player_touch(id, player);
    before - level.health(player)
}

#[test]
fn contact_damage_depends_on_size() {
    assert_eq!(touch_damage(sized(Slime::create(), 1)), 0.0);
    assert_eq!(touch_damage(sized(Slime::create(), 2)), 2.0);
    assert_eq!(touch_damage(sized(Slime::create(), 4)), 4.0);
    assert_eq!(touch_damage(sized(MagmaCube::create(), 1)), 3.0);
    assert_eq!(touch_damage(sized(MagmaCube::create(), 4)), 6.0);
}

#[test]
fn slimes_drop_slime_balls_only_when_tiny() {
    for (size, can_drop) in [(1, true), (2, false)] {
        let mut dropped = false;
        for seed in 0..20 {
            let mut level = testing::level();
            let mut mob = sized(Slime::create(), size);
            mob.entity.random = LegacyRandomSource::new(seed);
            let id = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
            level.hurt_server(id, DamageSource::new(damage_types::GENERIC_KILL), 100.0);
            dropped |= level.entities.values().filter_map(|e| e.as_item()).any(|item| item.item.get_item() == items::SLIME_BALL);
        }
        assert_eq!(dropped, can_drop, "size {size}");
    }
}
