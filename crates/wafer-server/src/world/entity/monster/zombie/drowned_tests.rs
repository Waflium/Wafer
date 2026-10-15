use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, items};
use wafer_protocol::nbt::Nbt;

use super::drowned::Drowned;
use super::husk::Husk;
use super::zombie::Zombie;
use crate::server::level::{ServerLevel, testing};
use crate::world::difficulty::Difficulty;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::update_flags::UPDATE_ALL;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn flood(level: &mut ServerLevel, from: BlockPos, to: BlockPos) {
    for pos in BlockPos::between_closed(from, to) {
        level.set_block(pos, blocks::WATER.default_state(), UPDATE_ALL);
    }
}

fn kind_of(level: &ServerLevel, entity_type: &str) -> Vec<EntityId> {
    level
        .entities
        .iter()
        .filter(|(_, e)| matches!(e, LevelEntity::Mob(m) if wafer_data::registry::ENTITY_TYPE[m.entity.entity_type as usize] == entity_type))
        .map(|(&id, _)| id)
        .collect()
}

#[test]
fn drowned_have_vanillas_attributes_and_navigation() {
    let mob = Drowned::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::StepHeight), 1.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 35.0);
    assert_eq!(attributes.value(Attribute::AttackDamage), 3.0);
    assert_eq!(mob.get_pathfinding_malus(crate::world::level::pathfinder::path_type::PathType::Water), 0.0);
    assert!(mob.ai.navigation.can_navigate_ground());
    assert_eq!(mob.ai.goal_selector.get_available_goals().len(), 9);
    assert_eq!(mob.ai.target_selector.get_available_goals().len(), 6);
}

#[test]
fn drowned_save_and_load_like_zombies() {
    let mut mob = Drowned::create();
    mob.set_baby(true);
    let mut out = Vec::new();
    mob.save(&mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("IsBaby"), Some(Nbt::Byte(1)));
    assert_eq!(get("DrownedConversionTime"), Some(Nbt::Int(-1)));
    let mut loaded = Drowned::create();
    loaded.load(&Nbt::Compound(out));
    assert!(loaded.is_baby());
    assert_eq!(loaded.entity.dimensions.height, 0.98);
}

#[test]
fn zombies_turn_into_drowned_under_water() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    flood(&mut level, BlockPos::new(-12, -60, -12), BlockPos::new(12, -56, 12));
    let mut mob = Zombie::create();
    mob.persistence_required = true;
    mob.entity.add_tag("converted");
    mob.equipment[EquipmentSlot::Mainhand as usize] = Some(ItemStack::new(items::IRON_SHOVEL, 1));
    Zombie::set_can_break_doors(&mut mob, true);
    let zombie = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    let mut ticks = 0;
    while level.entities.contains_key(&zombie) && ticks < 1000 {
        level.tick();
        ticks += 1;
    }
    let [drowned] = kind_of(&level, "minecraft:drowned")[..] else { panic!("no drowned after {ticks} ticks") };
    assert_eq!(ticks, 901);
    let mob = level.mob(drowned).unwrap();
    assert!(mob.entity.tags.iter().any(|t| t == "converted"));
    assert!(mob.item_by_slot(EquipmentSlot::Mainhand).is_some_and(|s| s.is(items::IRON_SHOVEL)));
    assert!(Zombie::can_break_doors(mob));
    assert!(mob.living.attributes.has_modifier(Attribute::KnockbackResistance, "minecraft:random_spawn_bonus"));
}

#[test]
fn husks_turn_into_zombies_under_water() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    flood(&mut level, BlockPos::new(-12, -60, -12), BlockPos::new(12, -56, 12));
    let mut mob = Husk::create();
    mob.persistence_required = true;
    let husk = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    for _ in 0..1000 {
        level.tick();
        if !level.entities.contains_key(&husk) {
            break;
        }
    }
    let [zombie] = kind_of(&level, "minecraft:zombie")[..] else { panic!("no zombie") };
    assert!(matches!(level.mob(zombie).unwrap().kind, MobKind::Zombie(_)));
}

#[test]
fn drowned_swim_towards_targets_in_water() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    flood(&mut level, BlockPos::new(-2, -60, -2), BlockPos::new(14, -54, 2));
    let player = testing::player(&mut level, Vec3::new(0.5, -58.0, 0.5), 0.0, 0.0);
    level.player_mut(player).abilities.flying = true;
    let drowned = add(&mut level, Drowned::create(), Vec3::new(12.5, -60.0, 0.5));
    for _ in 0..5 {
        level.tick();
        level.server_player_do_tick(player);
    }
    level.set_target(drowned, Some(player));
    let start = level.entity(drowned).position().distance_to(level.entity(player).position());
    let mut swam = false;
    for _ in 0..100 {
        level.tick();
        level.server_player_do_tick(player);
        swam |= level.entity(drowned).is_swimming();
    }
    let end = level.entity(drowned).position().distance_to(level.entity(player).position());
    assert!(swam, "the drowned never swam");
    assert!(end < start - 2.0, "the drowned did not approach: {start} -> {end}");
}

#[test]
fn drowned_with_tridents_throw_them() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    flood(&mut level, BlockPos::new(-2, -60, -2), BlockPos::new(10, -58, 2));
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = Drowned::create();
    mob.equipment[EquipmentSlot::Mainhand as usize] = Some(ItemStack::new(items::TRIDENT, 1));
    for _ in 0..70 {
        level.tick();
        level.server_player_do_tick(player);
    }
    let drowned = add(&mut level, mob, Vec3::new(8.5, -60.0, 0.5));
    level.set_target(drowned, Some(player));
    let mut thrown = false;
    for _ in 0..80 {
        level.tick();
        level.server_player_do_tick(player);
        thrown |= level.entities.values().any(|e| matches!(e, LevelEntity::Projectile(p) if matches!(p.kind, ProjectileKind::ThrownTrident(_))));
    }
    assert!(thrown, "the drowned never threw its trident");
    let mut hit = level.health(player) < 20.0;
    for _ in 0..200 {
        level.tick();
        level.server_player_do_tick(player);
        hit |= level.health(player) < 20.0;
    }
    let tridents = level.entities.values().filter(|e| matches!(e, LevelEntity::Projectile(p) if matches!(p.kind, ProjectileKind::ThrownTrident(_)))).count();
    assert!(tridents >= 3, "the drowned stopped throwing: {tridents} tridents");
    assert!(hit, "no trident ever hit");
}

#[test]
fn drowned_swimming_along_an_axis_turn_like_vanilla() {
    let expected = [
        (11.599410625002161, -61.998, 27.500000055127437, 89.999985),
        (11.598575236982644, -61.99940980635065, 27.50000010679455, 89.999985),
        (11.597019459355876, -61.99741194143276, 27.500000179539455, 89.999985),
        (11.59506972404235, -61.99542368794513, 27.500000270705, 89.999985),
        (11.592775851245003, -61.993448159081936, 27.500000377961697, 89.999985),
        (11.590181500360886, -61.99148784891335, 27.50000049926812, 89.999985),
        (11.587324918584123, -61.98954473234706, 27.50000049891943, 90.000046),
        (11.584239602376984, -61.98762035121429, 27.50000049854282, 90.000046),
    ];
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 30.5), 0.0, 0.0);
    flood(&mut level, BlockPos::new(-3, -63, 24), BlockPos::new(16, -57, 30));
    let drowned = add(&mut level, Drowned::create(), Vec3::new(11.6, -62.0, 27.5));
    let mob = level.mob_mut(drowned).unwrap();
    mob.entity.y_rot = 90.0;
    mob.entity.y_head_rot = 90.0;
    mob.living.y_body_rot = 90.0;
    mob.ai.goal_selector = Default::default();
    mob.ai.target_selector = Default::default();
    Drowned::set_searching_for_land(mob, true);
    for (x, y, z, y_rot) in expected {
        if level.mob(drowned).unwrap().ai.navigation.is_done() {
            level.get_navigation(drowned, |navigation, level| navigation.move_to(level, drowned, 0.5, -62.0, 27.5, 1.0));
        }
        level.tick();
        let e = level.entity(drowned);
        assert_eq!((e.x(), e.y(), e.z(), e.y_rot), (x, y, z, y_rot));
    }
}
