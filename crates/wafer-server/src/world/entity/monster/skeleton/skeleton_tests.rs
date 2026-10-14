use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, items, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::bogged::{self, Bogged};
use super::skeleton::Skeleton;
use super::stray::Stray;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::alchemy::potion_contents::PotionContents;
use crate::world::item::component as dc;
use crate::world::level::update_flags::UPDATE_ALL;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn armed(mut mob: Mob) -> Mob {
    mob.equipment[EquipmentSlot::Head as usize] = Some(ItemStack::new(items::LEATHER_HELMET, 1));
    mob
}

fn weapon_goal(level: &ServerLevel, id: EntityId) -> Vec<(i32, &'static str)> {
    let goals = level.mob(id).unwrap().ai.goal_selector.get_available_goals();
    goals
        .iter()
        .filter_map(|wrapped| match wrapped.get_goal() {
            Goal::RangedBowAttack(_) => Some((wrapped.get_priority(), "bow")),
            Goal::AbstractSkeletonMelee(_) => Some((wrapped.get_priority(), "melee")),
            _ => None,
        })
        .collect()
}

fn arrows(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.iter().filter(|(_, e)| matches!(e, LevelEntity::Projectile(p) if matches!(p.kind, ProjectileKind::Arrow(_)))).map(|(&id, _)| id).collect()
}

#[test]
fn skeletons_have_vanillas_attributes() {
    let mob = Skeleton::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 20.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 16.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.25);
    assert_eq!(attributes.value(Attribute::AttackDamage), 2.0);
    assert_eq!(mob.xp_reward, 5);
    assert_eq!(mob.entity.dimensions.height, 1.99);
    assert_eq!(mob.entity.dimensions.eye_height, 1.74);
    assert_eq!(Bogged::create().living.attributes.value(Attribute::MaxHealth), 16.0);
}

#[test]
fn skeletons_switch_between_melee_and_bow() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let skeleton = add(&mut level, Skeleton::create(), Vec3::new(0.5, -60.0, 0.5));
    let goals = level.mob(skeleton).unwrap().ai.goal_selector.get_available_goals();
    assert_eq!(goals.len(), 7);
    assert!(matches!(goals.last().unwrap().get_goal(), Goal::AbstractSkeletonMelee(_)));
    level.living_set_item_slot(skeleton, EquipmentSlot::Mainhand, ItemStack::new(items::BOW, 1));
    assert_eq!(weapon_goal(&level, skeleton), vec![(4, "bow")]);
    level.living_set_item_slot(skeleton, EquipmentSlot::Mainhand, ItemStack::new(items::IRON_SWORD, 1));
    assert_eq!(weapon_goal(&level, skeleton), vec![(4, "melee")]);
    level.living_set_item_slot(skeleton, EquipmentSlot::Offhand, ItemStack::new(items::BOW, 1));
    assert_eq!(weapon_goal(&level, skeleton), vec![(4, "bow")]);
    level.living_set_item_slot(skeleton, EquipmentSlot::Offhand, ItemStack::empty());
    assert_eq!(weapon_goal(&level, skeleton), vec![(4, "melee")]);
}

#[test]
fn finalize_spawn_gives_a_bow() {
    let mut level = testing::level();
    let difficulty = level.current_difficulty_at(BlockPos::ZERO);
    for mob in [Skeleton::create(), Stray::create(), Bogged::create()] {
        let (entity, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, EntitySpawnReason::Command, None);
        let LevelEntity::Mob(mob) = entity else { unreachable!() };
        assert!(mob.item_by_slot(EquipmentSlot::Mainhand).is_some_and(|bow| bow.is(items::BOW)));
        let goals = mob.ai.goal_selector.get_available_goals();
        assert!(matches!(goals.last().unwrap().get_goal(), Goal::RangedBowAttack(_)));
        assert!(mob.living.attributes.has_modifier(Attribute::FollowRange, "minecraft:random_spawn_bonus"));
    }
}

#[test]
fn skeletons_save_and_load_their_fields() {
    let mut level = testing::level();
    let mut mob = Skeleton::create();
    mob.equipment[EquipmentSlot::Mainhand as usize] = Some(ItemStack::new(items::BOW, 1));
    Skeleton::start_freeze_conversion(&mut mob, 120);
    let mut out = Vec::new();
    mob.save(&mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("StrayConversionTime"), Some(Nbt::Int(120)));
    assert_eq!(get("FreezingTime"), Some(Nbt::Int(-1)));
    let mut tag = vec![("id".to_string(), Nbt::String("minecraft:skeleton".into()))];
    tag.extend(out);
    let LevelEntity::Mob(loaded) = level.load_entity(&Nbt::Compound(tag)).unwrap() else { unreachable!() };
    assert!(matches!(loaded.ai.goal_selector.get_available_goals().last().unwrap().get_goal(), Goal::RangedBowAttack(_)));
    let mut out = Vec::new();
    loaded.save(&mut out);
    assert!(out.iter().any(|(k, v)| k == "StrayConversionTime" && *v == Nbt::Int(120)));

    let mut bogged = Bogged::create();
    bogged::set_sheared(&mut bogged.entity, true);
    let mut out = Vec::new();
    bogged.save(&mut out);
    assert!(out.iter().any(|(k, v)| k == "sheared" && *v == Nbt::Byte(1)));
    let mut loaded = Bogged::create();
    loaded.load(&Nbt::Compound(out));
    assert!(bogged::is_sheared(&loaded.entity));
}

fn shoot(level: &mut ServerLevel, mob: Mob) -> (EntityId, EntityId, Vec<EntityId>, usize) {
    level.difficulty = Difficulty::Normal;
    let player = testing::player(level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = armed(mob);
    mob.equipment[EquipmentSlot::Mainhand as usize] = Some(ItemStack::new(items::BOW, 1));
    let skeleton = add(level, mob, Vec3::new(10.5, -60.0, 0.5));
    level.reassess_weapon_goal(skeleton);
    level.set_target(skeleton, Some(player));
    for tick in 1..200 {
        level.tick();
        let found = arrows(level);
        if !found.is_empty() {
            return (player, skeleton, found, tick);
        }
    }
    panic!("the skeleton never shot");
}

#[test]
fn skeletons_draw_their_bow_for_twenty_ticks_and_shoot() {
    let mut level = testing::level();
    let (_, skeleton, arrows, tick) = shoot(&mut level, Skeleton::create());
    assert_eq!(arrows.len(), 1);
    assert!(tick >= 20, "shot after {tick} ticks");
    let arrow = level.projectile(arrows[0]);
    assert_eq!(level.projectile_get_owner(arrows[0]), Some(skeleton));
    assert!(arrow.abstract_arrow().unwrap().pickup_item_stack.get(dc::POTION_CONTENTS).is_none());
    assert!(!level.is_using_item(skeleton));
}

#[test]
fn strays_and_bogged_tip_their_arrows() {
    for (mob, effect, duration) in [(Stray::create(), mob_effects::SLOWNESS, 600), (Bogged::create(), mob_effects::POISON, 100)] {
        let mut level = testing::level();
        let (_, _, arrows, _) = shoot(&mut level, mob);
        let contents = level.projectile(arrows[0]).abstract_arrow().unwrap().pickup_item_stack.get(dc::POTION_CONTENTS).cloned().unwrap();
        let effects = PotionContents::new(&contents).for_each_effect(1.0);
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].effect, effect);
        assert_eq!(effects[0].duration, duration);
    }
}

#[test]
fn skeletons_freeze_into_strays_in_powder_snow() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 20.5), 0.0, 0.0);
    for y in -60..=-59 {
        level.set_block(BlockPos::new(0, y, 0), blocks::POWDER_SNOW.default_state(), UPDATE_ALL);
        for (x, z) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            level.set_block(BlockPos::new(x, y, z), blocks::STONE.default_state(), UPDATE_ALL);
        }
    }
    let mut mob = armed(Skeleton::create());
    mob.persistence_required = true;
    let skeleton = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    let mut converted = None;
    for _ in 0..500 {
        level.tick();
        if level.entity_ref(skeleton).is_none_or(|e| e.is_removed()) {
            converted = level.entities.iter().find(|(_, e)| matches!(e, LevelEntity::Mob(m) if matches!(m.kind, MobKind::Stray(_)))).map(|(&id, _)| id);
            break;
        }
        assert_eq!(level.entity(skeleton).ticks_frozen(), 0);
    }
    let stray = converted.expect("the skeleton never converted");
    assert!(level.item_by_slot(stray, EquipmentSlot::Head).is_some_and(|helmet| helmet.is(items::LEATHER_HELMET)));
}

#[test]
fn shearing_bogged_drops_mushrooms_once() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let bogged = add(&mut level, armed(Bogged::create()), Vec3::new(1.5, -60.0, 0.5));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::SHEARS, 1)));
    assert!(level.mob_interact(bogged, player, InteractionHand::MainHand).consumes_action());
    assert!(bogged::is_sheared(level.entity(bogged)));
    let mushrooms = level.entities.values().filter(|e| matches!(e, LevelEntity::Item(item) if item.item.is(items::RED_MUSHROOM) || item.item.is(items::BROWN_MUSHROOM))).count();
    assert!(mushrooms > 0);
    assert!(!level.mob_interact(bogged, player, InteractionHand::MainHand).consumes_action());
}
