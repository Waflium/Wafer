use wafer_core::{BlockPos, Vec3};
use wafer_data::properties::{EGGS, HATCH};
use wafer_data::{blocks, game_rule, items};
use wafer_protocol::nbt::Nbt;

use super::turtle::{self, Turtle};
use crate::server::level::{ServerLevel, testing};
use crate::world::difficulty::Difficulty;
use crate::world::entity::ageable_mob::BABY_START_AGE;
use crate::world::entity::mob::Mob;
use crate::world::entity::monster::zombie::zombie::Zombie;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::update_flags::UPDATE_ALL;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.set_pos(pos);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn items(level: &ServerLevel) -> Vec<(wafer_data::Item, i32)> {
    level.entities.values().filter_map(|entity| if let LevelEntity::Item(item) = entity { Some((item.item.item, item.item.count)) } else { None }).collect()
}

fn hatch_at_night(level: &mut ServerLevel) {
    let mut clocks = crate::world::clock::ServerClockManager::new();
    clocks.set_total_ticks(crate::world::clock::ServerClockManager::id_of("minecraft:overworld").unwrap(), 21500);
    level.sync_clocks(&clocks);
}

#[test]
fn hatching_eggs_release_baby_turtles_that_remember_the_beach() {
    let mut level = testing::level();
    hatch_at_night(&mut level);
    let pos = BlockPos::new(4, -60, 4);
    level.set_block(pos.below(), blocks::SAND.default_state(), UPDATE_ALL);
    let state = blocks::TURTLE_EGG.default_state().set(EGGS, 3).set(HATCH, 2);
    level.set_block(pos, state, UPDATE_ALL);
    block::of(state).random_tick(state, &mut level, pos);
    assert!(level.block_state(pos).is_air());
    let babies: Vec<&Mob> = level.entities.values().filter_map(|entity| if let LevelEntity::Mob(mob) = entity { Some(&**mob) } else { None }).collect();
    assert_eq!(babies.len(), 3);
    for (i, baby) in babies.iter().enumerate() {
        assert!(baby.is_baby());
        assert_eq!(baby.ageable().unwrap().age, BABY_START_AGE);
        assert_eq!(turtle::get_home_pos(baby), pos);
        assert_eq!(baby.entity.position(), Vec3::new(4.3 + i as f64 * 0.2, -60.0, 4.3));
    }
}

#[test]
fn eggs_off_the_sand_never_hatch() {
    let mut level = testing::level();
    hatch_at_night(&mut level);
    let pos = BlockPos::new(4, -60, 4);
    let state = blocks::TURTLE_EGG.default_state().set(HATCH, 2);
    level.set_block(pos, state, UPDATE_ALL);
    for _ in 0..20 {
        let state = level.block_state(pos);
        block::of(state).random_tick(state, &mut level, pos);
    }
    assert_eq!(level.block_state(pos), state);
}

#[test]
fn growing_up_drops_a_scute_unless_mob_drops_are_off() {
    for mob_drops in [true, false] {
        let mut level = testing::level();
        testing::player(&mut level, Vec3::new(0.5, -60.0, 80.5), 0.0, 0.0);
        level.game_rules.set(game_rule::MOB_DROPS, mob_drops as i32);
        let mut mob = Turtle::create();
        mob.set_age(-1);
        let turtle = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
        level.tick();
        assert!(!level.mob(turtle).unwrap().is_baby());
        let expected = if mob_drops { vec![(items::TURTLE_SCUTE, 1)] } else { Vec::new() };
        assert_eq!(items(&level), expected);
    }
}

#[test]
fn home_and_egg_survive_saving() {
    let mut mob = Turtle::create();
    mob.entity.set_pos(Vec3::new(10.5, 64.0, -3.5));
    turtle::set_home_pos(&mut mob, BlockPos::new(1, 2, 3));
    mob.entity.data.set(turtle::HAS_EGG, crate::world::entity::data::DataValue::Boolean(true));
    let mut out = Vec::new();
    mob.save(&mut out);
    assert_eq!(out.iter().filter(|(key, _)| key == "home_pos").count(), 1);
    let nbt = Nbt::Compound(out.into());
    assert_eq!(nbt.get("home_pos"), Some(&Nbt::IntArray(vec![1, 2, 3])));
    assert_eq!(nbt.get("has_egg"), Some(&Nbt::Byte(1)));
    let mut loaded = Turtle::create();
    loaded.load(&nbt);
    assert_eq!(turtle::get_home_pos(&loaded), BlockPos::new(1, 2, 3));
    assert!(turtle::has_egg(&loaded.entity));
    let mut homeless = Turtle::create();
    homeless.entity.set_pos(Vec3::new(10.5, 64.0, -3.5));
    homeless.load(&Nbt::Compound(Vec::new().into()));
    assert_eq!(turtle::get_home_pos(&homeless), BlockPos::new(10, 64, -4));
}

#[test]
fn turtles_carrying_eggs_cannot_fall_in_love() {
    let mut level = testing::level();
    let turtle = add(&mut level, Turtle::create(), Vec3::new(0.5, -60.0, 0.5));
    assert!(level.can_fall_in_love(turtle));
    level.entity_mut(turtle).data.set(turtle::HAS_EGG, crate::world::entity::data::DataValue::Boolean(true));
    assert!(!level.can_fall_in_love(turtle));
}

#[test]
fn zombies_hunt_baby_turtles_on_land() {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 80.5), 0.0, 0.0);
    level.difficulty = Difficulty::Normal;
    let mut baby = Turtle::create();
    baby.set_age(BABY_START_AGE);
    baby.refresh_dimensions();
    let baby = add(&mut level, baby, Vec3::new(4.5, -60.0, 0.5));
    let adult = add(&mut level, Turtle::create(), Vec3::new(-3.5, -60.0, 0.5));
    let zombie = add(&mut level, Zombie::create(), Vec3::new(0.5, -60.0, 0.5));
    let mut target = None;
    for _ in 0..200 {
        level.tick();
        target = level.mob(zombie).and_then(|mob| mob.target);
        if target.is_some() {
            break;
        }
    }
    assert_eq!(target, Some(baby));
    assert_ne!(target, Some(adult));
}

#[test]
fn turtles_are_not_pushed_by_water() {
    let mut level = testing::level();
    let turtle = add(&mut level, Turtle::create(), Vec3::new(0.5, -60.0, 0.5));
    assert!(!level.is_pushed_by_fluid(turtle));
}
