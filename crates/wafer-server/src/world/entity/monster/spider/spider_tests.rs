use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, mob_effects};

use super::cave_spider::CaveSpider;
use super::spider::{Spider, SpiderEffectsGroupData};
use crate::server::level::{ServerLevel, testing};
use crate::world::clock::ServerClockManager;
use crate::world::difficulty::Difficulty;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::Mob;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::scores::team::CollisionRule;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn level_at(time: i64) -> ServerLevel {
    let mut level = testing::level();
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), time);
    level.sync_clocks(&clocks);
    level.difficulty = Difficulty::Normal;
    level
}

fn oracle(text: &str, start: Vec3) -> Vec<Vec3> {
    text.lines()
        .map(|line| {
            let v: Vec<f64> = line.split(' ').map(|n| n.parse().unwrap()).collect();
            Vec3::new(v[0], v[1], v[2])
        })
        .skip_while(|pos| *pos == start)
        .collect()
}

fn unpushable_player(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let player = testing::player(level, pos, 0.0, 0.0);
    let mut scoreboard = level.scoreboard.borrow_mut();
    scoreboard.add_player_team("t");
    scoreboard.set_collision_rule("t", CollisionRule::Never);
    scoreboard.add_player_to_team_base("Tester", "t");
    drop(scoreboard);
    player
}

fn path_from_first_move(level: &mut ServerLevel, spider: EntityId, start: Vec3, ticks: usize) -> Vec<Vec3> {
    let mut path = Vec::new();
    for _ in 0..300 {
        level.tick();
        let pos = level.entity(spider).position();
        if pos != start || !path.is_empty() {
            path.push(pos);
        }
        if path.len() == ticks {
            break;
        }
    }
    path
}

#[test]
fn spiders_have_vanillas_attributes() {
    let mob = Spider::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 16.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.3f32 as f64);
    assert_eq!(attributes.value(Attribute::AttackDamage), 2.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 16.0);
    assert_eq!(mob.xp_reward, 5);
    assert_eq!((mob.entity.dimensions.width, mob.entity.dimensions.height, mob.entity.dimensions.eye_height), (1.4, 0.9, 0.65));
    let mob = CaveSpider::create();
    assert_eq!(mob.living.attributes.value(Attribute::MaxHealth), 12.0);
    assert_eq!((mob.entity.dimensions.width, mob.entity.dimensions.height, mob.entity.dimensions.eye_height), (0.7, 0.5, 0.45));
}

#[test]
fn chasing_spiders_leap_like_vanilla() {
    let (actual, expected) = chase(CHASE_PARITY, CHASE_SEED);
    assert!(expected.iter().any(|pos| pos.y > -60.0), "the oracle has no leap");
    assert_eq!(actual, expected);
}

#[test]
fn spiders_climb_walls_like_vanilla() {
    let (actual, expected) = climb(CLIMB_PARITY, CLIMB_SEED);
    assert_eq!(actual, expected);
    assert_eq!(actual.last().map(|pos| pos.y), Some(-54.0));
}

const CHASE_PARITY: i32 = 0;
const CHASE_SEED: i64 = 3;
const CLIMB_PARITY: i32 = 0;
const CLIMB_SEED: i64 = 0;

fn spider_with(parity: i32, seed: i64) -> Mob {
    loop {
        let mut mob = Spider::create();
        if mob.entity.id.rem_euclid(2) == parity {
            mob.entity.random = LegacyRandomSource::new(seed);
            return mob;
        }
    }
}

fn chase(parity: i32, seed: i64) -> (Vec<Vec3>, Vec<Vec3>) {
    let start = Vec3::new(8.5, -60.0, 0.5);
    let expected = oracle(include_str!("chase_oracle.txt"), start);
    let mut level = level_at(18000);
    let player = unpushable_player(&mut level, Vec3::new(0.5, -60.0, 0.5));
    let spider = add(&mut level, spider_with(parity, seed), start);
    level.set_target(spider, Some(player));
    (path_from_first_move(&mut level, spider, start, expected.len()), expected)
}

fn climb(parity: i32, seed: i64) -> (Vec<Vec3>, Vec<Vec3>) {
    let mut level = level_at(18000);
    for y in -60..=-55 {
        for z in -3..=3 {
            level.set_block(BlockPos::new(4, y, z), blocks::STONE.default_state(), UPDATE_ALL);
        }
    }
    let player = unpushable_player(&mut level, Vec3::new(4.5, -54.0, 0.5));
    let start = Vec3::new(10.5, -60.0, 0.5);
    let expected = oracle(include_str!("climb_oracle.txt"), start);
    let spider = add(&mut level, spider_with(parity, seed), start);
    level.set_target(spider, Some(player));
    (path_from_first_move(&mut level, spider, start, expected.len()), expected)
}

#[test]
fn spiders_are_neutral_in_daylight() {
    let mut level = level_at(6000);
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let spider = add(&mut level, Spider::create(), Vec3::new(4.5, -60.0, 0.5));
    for _ in 0..200 {
        level.tick();
        assert_eq!(level.get_target(spider), None);
    }
    assert_eq!(level.health(player), 20.0);
    let mut level = level_at(18000);
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let spider = add(&mut level, Spider::create(), Vec3::new(4.5, -60.0, 0.5));
    for _ in 0..100 {
        level.tick();
    }
    assert_eq!(level.get_target(spider), Some(player));
}

#[test]
fn spiders_give_up_attacking_in_daylight() {
    let mut level = level_at(6000);
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 20.5), 0.0, 0.0);
    let mut mob = Spider::create();
    mob.entity.random = LegacyRandomSource::new(1);
    let spider = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    level.set_target(spider, Some(player));
    let mut ticks = 0;
    while level.get_target(spider).is_some() {
        level.tick();
        ticks += 1;
        assert!(ticks < 2000, "the spider never gave up");
    }
}

#[test]
fn spiders_ignore_cobwebs_and_poison() {
    let mut level = level_at(6000);
    let spider = add(&mut level, Spider::create(), Vec3::new(0.5, -60.0, 0.5));
    let cobweb = blocks::COBWEB.default_state();
    level.make_stuck_in_block(spider, cobweb, Vec3::new(0.25, 0.05, 0.25));
    assert_eq!(level.entity(spider).stuck_speed_multiplier, Vec3::ZERO);
    level.make_stuck_in_block(spider, blocks::SWEET_BERRY_BUSH.default_state(), Vec3::new(0.8, 0.75, 0.8));
    assert_eq!(level.entity(spider).stuck_speed_multiplier, Vec3::new(0.8, 0.75, 0.8));
    assert!(!level.add_effect(spider, MobEffectInstance::new(mob_effects::POISON, 100, 0), None));
    assert!(level.add_effect(spider, MobEffectInstance::new(mob_effects::REGENERATION, 100, 0), None));
    let cave_spider = add(&mut level, CaveSpider::create(), Vec3::new(4.5, -60.0, 0.5));
    assert!(!level.add_effect(cave_spider, MobEffectInstance::new(mob_effects::POISON, 100, 0), None));
}

#[test]
fn cave_spiders_poison_by_difficulty() {
    for (difficulty, duration) in [(Difficulty::Easy, None), (Difficulty::Normal, Some(140)), (Difficulty::Hard, Some(300))] {
        let mut level = level_at(6000);
        level.difficulty = difficulty;
        let cow = add(&mut level, crate::world::entity::animal::cow::cow::Cow::create(), Vec3::new(0.5, -60.0, 0.5));
        let cave_spider = add(&mut level, CaveSpider::create(), Vec3::new(1.5, -60.0, 0.5));
        assert!(level.do_hurt_target(cave_spider, cow));
        assert_eq!(level.living(cow).effect(mob_effects::POISON).map(|effect| effect.duration), duration);
    }
}

#[test]
fn spider_effects_group_data_applies_an_infinite_effect() {
    let mut level = level_at(18000);
    let difficulty = level.current_difficulty_at(BlockPos::ZERO);
    let group_data = Some(SpawnGroupData::SpiderEffects(SpiderEffectsGroupData { effect: Some(mob_effects::INVISIBILITY) }));
    let (entity, returned) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(Spider::create())), &difficulty, EntitySpawnReason::Command, group_data);
    let LevelEntity::Mob(mob) = entity else { unreachable!() };
    assert_eq!(returned, group_data);
    assert_eq!(mob.living.effect(mob_effects::INVISIBILITY).map(|effect| effect.duration), Some(-1));
    assert!(mob.living.attributes.has_modifier(Attribute::FollowRange, "minecraft:random_spawn_bonus"));
    let (entity, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(Spider::create())), &difficulty, EntitySpawnReason::Command, None);
    let LevelEntity::Mob(mob) = entity else { unreachable!() };
    assert!(mob.living.active_effects.is_empty());
}

#[test]
fn cave_spiders_skip_mob_finalize_spawn() {
    let mut level = level_at(18000);
    let difficulty = level.current_difficulty_at(BlockPos::ZERO);
    let (entity, group_data) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(CaveSpider::create())), &difficulty, EntitySpawnReason::Command, None);
    let LevelEntity::Mob(mob) = entity else { unreachable!() };
    assert_eq!(group_data, None);
    assert!(!mob.living.attributes.has_modifier(Attribute::FollowRange, "minecraft:random_spawn_bonus"));
}

#[test]
fn spider_random_effects_follow_vanillas_table() {
    let mut random = LegacyRandomSource::new(0);
    let mut seen = Vec::new();
    for _ in 0..200 {
        let mut data = SpiderEffectsGroupData::default();
        data.set_random_effect(&mut random);
        let effect = data.effect.expect("an effect");
        if !seen.contains(&effect) {
            seen.push(effect);
        }
    }
    seen.sort();
    let mut expected = vec![mob_effects::SPEED, mob_effects::STRENGTH, mob_effects::REGENERATION, mob_effects::INVISIBILITY];
    expected.sort();
    assert_eq!(seen, expected);
}

#[test]
fn spiders_sit_low_on_narrower_vehicles() {
    let mut level = level_at(6000);
    let spider = add(&mut level, Spider::create(), Vec3::new(0.5, -60.0, 0.5));
    let cave_spider = add(&mut level, CaveSpider::create(), Vec3::new(4.5, -60.0, 0.5));
    let chicken = add(&mut level, crate::world::entity::animal::chicken::Chicken::create(), Vec3::new(8.5, -60.0, 0.5));
    assert_eq!(level.vehicle_attachment_point(spider, chicken), Vec3::new(0.0, 0.3125, 0.0));
    assert_eq!(level.vehicle_attachment_point(cave_spider, chicken), Vec3::new(0.0, 0.21875, 0.0));
    assert_eq!(level.vehicle_attachment_point(cave_spider, spider), level.entity_get_vehicle_attachment_point(cave_spider, spider));
}
