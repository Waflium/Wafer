use wafer_core::random::LegacyRandomSource;
use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, damage_types, items};
use wafer_protocol::nbt::Nbt;

use super::iron_golem::{self, IronGolem};
use super::snow_golem::{self, SnowGolem};
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::clock::ServerClockManager;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::crackiness::Level;
use crate::world::entity::mob::Mob;
use crate::world::entity::monster::zombie::zombie::Zombie;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

const ORACLE: &str = include_str!("golem_oracle.txt");

fn night_level(player_pos: Vec3) -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    let mut clocks = ServerClockManager::new();
    clocks.set_total_ticks(ServerClockManager::id_of("minecraft:overworld").unwrap(), 18000);
    level.sync_clocks(&clocks);
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, player_pos, 0.0, 0.0);
    let mut scoreboard = level.scoreboard.borrow_mut();
    scoreboard.add_player_team("t");
    scoreboard.set_collision_rule("t", crate::world::scores::team::CollisionRule::Never);
    scoreboard.add_player_to_team_base("Tester", "t");
    drop(scoreboard);
    for _ in 0..61 {
        level.tick();
    }
    (level, player)
}

fn spawn(level: &mut ServerLevel, create: fn() -> Mob, parity: i32, pos: Vec3, seed: i64) -> EntityId {
    let mut mob = loop {
        let mob = create();
        if mob.entity.id.rem_euclid(2) == parity {
            break mob;
        }
    };
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    mob.entity.y_head_rot = 0.0;
    mob.living.y_body_rot = 0.0;
    mob.persistence_required = true;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    level.mob_mut(id).unwrap().entity.random = LegacyRandomSource::new(seed);
    id
}

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn oracle_section(label: &str) -> Vec<&'static str> {
    ORACLE.lines().skip_while(|line| *line != format!("== {label}")).skip(1).take_while(|line| !line.starts_with("==")).filter(|line| !line.starts_with("spawned")).collect()
}

fn type_path(level: &ServerLevel, id: EntityId) -> &'static str {
    wafer_data::registry::ENTITY_TYPE[level.entity(id).entity_type as usize].trim_start_matches("minecraft:")
}

fn assert_mob_row(level: &ServerLevel, id: EntityId, expected: &str, context: &str) {
    let fields: Vec<&str> = expected.split(' ').collect();
    if fields[1] == "gone" {
        assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()), "{context}: expected gone");
        return;
    }
    let e = level.entity(id);
    let actual = (e.x(), e.y(), e.z(), e.delta_movement.y);
    let wanted: (f64, f64, f64, f64) = (fields[1].parse().unwrap(), fields[2].parse().unwrap(), fields[3].parse().unwrap(), fields[4].parse().unwrap());
    assert_eq!(actual, wanted, "{context}: position");
    assert_eq!(level.health(id), fields[5].parse::<f32>().unwrap(), "{context}: health");
    let target = level.get_target(id).map_or("-", |target| type_path(level, target));
    assert_eq!(target, fields[6], "{context}: target");
    if fields.len() > 7 {
        let end = level.get_persistent_anger_end_time(id);
        let remaining = if end > 0 { end - level.game_time } else { -1 };
        assert_eq!(remaining, fields[7].parse::<i64>().unwrap(), "{context}: anger");
    }
}

fn segments(row: &str) -> Vec<&str> {
    row.split(" | ").map(str::trim).filter(|segment| !segment.is_empty()).collect()
}

#[test]
fn golems_have_vanillas_attributes() {
    let iron_golem = IronGolem::create();
    let attributes = &iron_golem.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 100.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.25);
    assert_eq!(attributes.value(Attribute::KnockbackResistance), 1.0);
    assert_eq!(attributes.value(Attribute::AttackDamage), 15.0);
    assert_eq!(attributes.value(Attribute::StepHeight), 1.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 16.0);
    assert_eq!((iron_golem.entity.dimensions.width, iron_golem.entity.dimensions.height), (1.4, 2.7));
    let snow_golem = SnowGolem::create();
    let attributes = &snow_golem.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 4.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.2f32 as f64);
    assert_eq!((snow_golem.entity.dimensions.width, snow_golem.entity.dimensions.height, snow_golem.entity.dimensions.eye_height), (0.7, 1.9, 1.7));
    assert!(snow_golem::has_pumpkin(&snow_golem.entity));
    assert!(!iron_golem::is_player_created(&iron_golem.entity));
}

#[test]
fn iron_golems_fight_zombies_like_vanilla() {
    let (mut level, _) = night_level(Vec3::new(0.5, -60.0, -40.5));
    let golem = spawn(&mut level, IronGolem::create, 1, Vec3::new(0.5, -60.0, 0.5), 42);
    let zombie = spawn(&mut level, Zombie::create, 0, Vec3::new(8.5, -60.0, 0.5), 7);
    let rows = oracle_section("zombie");
    assert_eq!(rows.len(), 160);
    for (tick, row) in rows.iter().enumerate() {
        level.tick();
        let segments = segments(row);
        assert_mob_row(&level, golem, segments[0], &format!("golem tick {}", tick + 1));
        assert_mob_row(&level, zombie, segments[1], &format!("zombie tick {}", tick + 1));
    }
}

#[test]
fn iron_golems_attack_players_who_hit_them_like_vanilla() {
    let (mut level, player) = night_level(Vec3::new(3.5, -60.0, 100.5));
    let golem = spawn(&mut level, IronGolem::create, 0, Vec3::new(0.5, -60.0, 100.5), 5);
    let rows = oracle_section("anger");
    for (tick, row) in rows.iter().enumerate() {
        if tick == 3 {
            let source = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
            level.hurt_server(golem, source, 1.0);
        }
        level.tick();
        let segments = segments(row);
        let player_health: f32 = segments[1].split(' ').nth(1).unwrap().parse().unwrap();
        assert_eq!(level.health(player), player_health, "player health tick {}", tick + 1);
        if player_health <= 0.0 {
            break;
        }
        assert_mob_row(&level, golem, segments[0], &format!("golem tick {}", tick + 1));
    }
    assert!(level.health(player) <= 0.0);
}

#[test]
fn snow_golems_wander_and_leave_snow_like_vanilla() {
    let (mut level, _) = night_level(Vec3::new(0.5, -60.0, 20.5));
    let golem = spawn(&mut level, SnowGolem::create, 1, Vec3::new(0.5, -60.0, 40.5), 21);
    let section = oracle_section("snow");
    let (rows, snow): (Vec<&str>, Vec<&str>) = section.into_iter().partition(|line| !line.starts_with("snow "));
    assert_eq!(rows.len(), 600);
    assert!(snow.len() > 20);
    for (tick, row) in rows.iter().enumerate() {
        level.tick();
        assert_mob_row(&level, golem, segments(row)[0], &format!("snow golem tick {}", tick + 1));
    }
    let mut actual = Vec::new();
    for x in -20..=20 {
        for z in 20..=80 {
            if level.block_state(BlockPos::new(x, -60, z)).is_block(blocks::SNOW) {
                actual.push(format!("snow {x} {z}"));
            }
        }
    }
    assert_eq!(actual, snow);
}

#[test]
fn snow_golems_throw_snowballs_like_vanilla() {
    let (mut level, _) = night_level(Vec3::new(0.5, -60.0, 20.5));
    let golem = spawn(&mut level, SnowGolem::create, 0, Vec3::new(40.5, -60.0, 60.5), 13);
    let zombie = spawn(&mut level, Zombie::create, 1, Vec3::new(49.5, -60.0, 60.5), 3);
    let rows = oracle_section("snowfight");
    let mut seen = Vec::new();
    for (tick, row) in rows.iter().enumerate() {
        level.tick();
        let segments = segments(row);
        assert_mob_row(&level, golem, segments[0], &format!("snow golem tick {}", tick + 1));
        assert_mob_row(&level, zombie, segments[1], &format!("zombie tick {}", tick + 1));
        let snowballs: Vec<EntityId> = level
            .entities
            .iter()
            .filter(|(id, e)| matches!(e, LevelEntity::Projectile(_)) && type_path(&level, **id) == "snowball" && !seen.contains(*id))
            .map(|(id, _)| *id)
            .collect();
        let expected: Vec<&str> = segments.iter().filter(|segment| segment.starts_with("snowball")).copied().collect();
        assert_eq!(snowballs.len(), expected.len(), "snowballs tick {}", tick + 1);
        if let Some(&snowball) = snowballs.first() {
            let fields: Vec<f64> = expected[0].split(' ').skip(1).map(|n| n.parse().unwrap()).collect();
            let e = level.entity(snowball);
            assert_eq!((e.x(), e.y(), e.z()), (fields[0], fields[1], fields[2]), "snowball tick {}", tick + 1);
            return;
        }
        seen.extend(snowballs);
    }
    panic!("no snowball thrown");
}

fn build_level() -> ServerLevel {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level
}

#[test]
fn player_created_iron_golems_never_attack_players_or_creepers() {
    let mut level = build_level();
    let player = *level.players.keys().next().unwrap();
    let mut golem = IronGolem::create();
    iron_golem::set_player_created(&mut golem.entity, true);
    let golem = add(&mut level, golem, Vec3::new(3.5, -60.0, 0.5));
    let creeper = add(&mut level, crate::world::entity::monster::creeper::Creeper::create(), Vec3::new(6.5, -60.0, 0.5));
    let zombie = add(&mut level, Zombie::create(), Vec3::new(9.5, -60.0, 0.5));
    assert!(!level.can_attack(golem, player));
    assert!(!level.can_attack(golem, creeper));
    assert!(level.can_attack(golem, zombie));
}

#[test]
fn iron_golems_crack_and_are_repaired_with_iron_ingots() {
    let mut level = build_level();
    let player = *level.players.keys().next().unwrap();
    let golem = add(&mut level, IronGolem::create(), Vec3::new(3.5, -60.0, 0.5));
    let crackiness = |level: &ServerLevel| IronGolem::get_crackiness(level.mob(golem).unwrap());
    assert_eq!(crackiness(&level), Level::None);
    level.set_health(golem, 74.0);
    assert_eq!(crackiness(&level), Level::Low);
    level.set_health(golem, 49.0);
    assert_eq!(crackiness(&level), Level::Medium);
    level.set_health(golem, 24.0);
    assert_eq!(crackiness(&level), Level::High);
    level.set_item_slot(player, EquipmentSlot::Mainhand, ItemStack::new(items::IRON_INGOT, 2));
    assert!(level.mob_interact(golem, player, InteractionHand::MainHand).consumes_action());
    assert_eq!(level.health(golem), 49.0);
    assert_eq!(level.player(player).item_in_hand(InteractionHand::MainHand).map(|stack| stack.count), Some(1));
    level.set_health(golem, 100.0);
    assert!(!level.mob_interact(golem, player, InteractionHand::MainHand).consumes_action());
    assert_eq!(level.player(player).item_in_hand(InteractionHand::MainHand).map(|stack| stack.count), Some(1));
}

#[test]
fn shearing_a_snow_golem_drops_its_pumpkin() {
    let mut level = build_level();
    let player = *level.players.keys().next().unwrap();
    let golem = add(&mut level, SnowGolem::create(), Vec3::new(3.5, -60.0, 0.5));
    level.set_item_slot(player, EquipmentSlot::Mainhand, ItemStack::new(items::SHEARS, 1));
    assert!(level.mob_interact(golem, player, InteractionHand::MainHand).consumes_action());
    assert!(!snow_golem::has_pumpkin(level.entity(golem)));
    let drops: Vec<(i32, f64)> =
        level.entities.values().filter_map(|e| match e { LevelEntity::Item(item) => Some((item.item.item.0 as i32, item.entity.y())), _ => None }).collect();
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0].0, items::CARVED_PUMPKIN.0 as i32);
    assert!(!level.mob_interact(golem, player, InteractionHand::MainHand).consumes_action());
}

#[test]
fn golems_save_and_load_their_fields() {
    let mut golem = IronGolem::create();
    golem.load(&Nbt::Compound(vec![("PlayerCreated".into(), Nbt::Byte(1)), ("Health".into(), Nbt::Float(60.0))]));
    let mut out = Vec::new();
    golem.save(&mut out);
    assert!(out.iter().any(|(key, value)| key == "PlayerCreated" && *value == Nbt::Byte(1)));
    assert!(out.iter().any(|(key, _)| key == "anger_end_time"));
    let mut snow_golem = SnowGolem::create();
    snow_golem.load(&Nbt::Compound(vec![("Pumpkin".into(), Nbt::Byte(0))]));
    let mut out = Vec::new();
    snow_golem.save(&mut out);
    assert!(out.iter().any(|(key, value)| key == "Pumpkin" && *value == Nbt::Byte(0)));
}

#[test]
fn killed_golems_drop_their_loot() {
    let mut level = build_level();
    let iron_golem = add(&mut level, IronGolem::create(), Vec3::new(3.5, -60.0, 0.5));
    let snow_golem = add(&mut level, SnowGolem::create(), Vec3::new(-3.5, -60.0, 0.5));
    level.kill(iron_golem);
    level.kill(snow_golem);
    let mut iron_ingots = 0;
    for e in level.entities.values() {
        if let LevelEntity::Item(item) = e {
            let item_stack = &item.item;
            assert!(item_stack.is(items::IRON_INGOT) || item_stack.is(items::POPPY) || item_stack.is(items::SNOWBALL), "{:?}", item_stack.item);
            if item_stack.is(items::IRON_INGOT) {
                iron_ingots += item_stack.count;
            }
        }
    }
    assert!((3..=5).contains(&iron_ingots));
}

#[test]
fn slimes_and_iron_golems_fight() {
    let (mut level, _) = night_level(Vec3::new(0.5, -60.0, -20.5));
    let golem = spawn(&mut level, IronGolem::create, 1, Vec3::new(0.5, -60.0, 0.5), 3);
    let mut slime = crate::world::entity::monster::cubemob::slime::Slime::create();
    slime.set_size(3, true);
    let slime = add(&mut level, slime, Vec3::new(5.5, -60.0, 0.5));
    level.mob_mut(slime).unwrap().entity.random = LegacyRandomSource::new(5);
    let mut targeted = (false, false);
    for _ in 0..200 {
        level.tick();
        targeted.0 |= level.get_target(slime) == Some(golem);
        targeted.1 |= level.get_target(golem) == Some(slime);
    }
    assert_eq!(targeted, (true, true));
}

#[test]
fn iron_golems_take_no_fall_damage_and_dont_drown() {
    let mut level = build_level();
    let mut mob = IronGolem::create();
    mob.set_no_ai(true);
    let golem = add(&mut level, mob, Vec3::new(3.5, -60.0, 3.5));
    assert!(!level.hurt_server(golem, DamageSource::new(damage_types::FALL), 10.0));
    assert_eq!(level.health(golem), 100.0);
    for x in 1..=6 {
        for y in -60..=-55 {
            for z in 1..=6 {
                level.set_block(BlockPos::new(x, y, z), blocks::WATER.default_state(), UPDATE_ALL);
            }
        }
    }
    for _ in 0..400 {
        level.tick();
    }
    assert!(level.entity(golem).is_eye_in_fluid(&wafer_data::fluid_tags::WATER));
    assert_eq!(level.entity(golem).air_supply(), 300);
    assert_eq!(level.health(golem), 100.0);
}
