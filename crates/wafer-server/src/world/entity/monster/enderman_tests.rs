use wafer_core::random::LegacyRandomSource;
use wafer_core::{AABB, BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, damage_types, game_rule, items};
use wafer_protocol::nbt::Nbt;

use super::enderman::{self, Enderman, EndermanLeaveBlockGoal, EndermanTakeBlockGoal};
use crate::server::level::{ServerLevel, testing};
use crate::world::clock::ServerClockManager;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::entity_reference::EntityReference;
use crate::world::entity::mob::Mob;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

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

fn enderman_with(parity: i32, seed: i64) -> Mob {
    loop {
        let mut mob = Enderman::create();
        if mob.entity.id.rem_euclid(2) == parity {
            mob.entity.random = LegacyRandomSource::new(seed);
            return mob;
        }
    }
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

fn level_with_bystander() -> ServerLevel {
    let mut level = level_at(18000);
    testing::player(&mut level, Vec3::new(0.5, -60.0, 30.5), 0.0, 0.0);
    level
}

fn count_blocks(level: &mut ServerLevel, block: wafer_data::Block, from: BlockPos, to: BlockPos) -> usize {
    BlockPos::between_closed(from, to).filter(|&pos| level.block_state(pos).is_block(block)).count()
}

#[test]
fn endermen_have_vanillas_attributes() {
    let mob = Enderman::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 40.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.3f32 as f64);
    assert_eq!(attributes.value(Attribute::AttackDamage), 7.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 64.0);
    assert_eq!(attributes.value(Attribute::StepHeight), 1.0);
    assert_eq!(mob.xp_reward, 5);
    assert_eq!((mob.entity.dimensions.width, mob.entity.dimensions.height, mob.entity.dimensions.eye_height), (0.6, 2.9, 2.55));
}

const CHASE_PARITY: i32 = 0;
const CHASE_SEED: i64 = 0;

#[test]
fn stared_at_endermen_freeze_then_chase_like_vanilla() {
    let mut level = level_at(18000);
    level.set_block(BlockPos::new(0, -60, 0), blocks::STONE.default_state(), UPDATE_ALL);
    let player = testing::player(&mut level, Vec3::new(0.5, -59.0, 0.5), 0.0, 0.0);
    let start = Vec3::new(0.5, -60.0, 8.5);
    let enderman = add(&mut level, enderman_with(CHASE_PARITY, CHASE_SEED), start);
    for _ in 0..30 {
        level.tick();
        assert_eq!(level.entity(enderman).position(), start);
    }
    assert_eq!(level.get_target(enderman), Some(player));
    assert!(enderman::is_creepy(level.entity(enderman)));
    assert_eq!(level.attribute_value(enderman, Attribute::MovementSpeed), 0.3f32 as f64 + 0.15f32 as f64);
    level.set_item_slot(player, EquipmentSlot::Head, ItemStack::new(items::CARVED_PUMPKIN, 1));
    let expected = oracle(include_str!("enderman_chase_oracle.txt"), start);
    let mut path = Vec::new();
    for _ in 0..100 {
        level.tick();
        let pos = level.entity(enderman).position();
        if pos != start || !path.is_empty() {
            path.push(pos);
        }
        if path.len() == expected.len() {
            break;
        }
    }
    assert_eq!(path, expected);
}

#[test]
fn carved_pumpkins_hide_stares() {
    let mut level = level_at(18000);
    let player = testing::player(&mut level, Vec3::new(0.5, -59.0, 0.5), 0.0, 0.0);
    level.set_item_slot(player, EquipmentSlot::Head, ItemStack::new(items::CARVED_PUMPKIN, 1));
    let enderman = add(&mut level, Enderman::create(), Vec3::new(0.5, -60.0, 8.5));
    assert!(!level.is_being_stared_by(enderman, player));
    level.set_item_slot(player, EquipmentSlot::Head, ItemStack::empty());
    assert!(level.is_being_stared_by(enderman, player));
    level.entity_mut(player).y_head_rot = 90.0;
    assert!(!level.is_being_stared_by(enderman, player));
}

#[test]
fn endermen_stay_angry_until_their_target_dies() {
    let mut level = level_at(18000);
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 30.5), 0.0, 0.0);
    let enderman = add(&mut level, Enderman::create(), Vec3::new(0.5, -60.0, 0.5));
    assert!(!level.is_angry(enderman));
    level.set_target(enderman, Some(player));
    level.update_persistent_anger(enderman, true);
    let remaining = level.get_persistent_anger_end_time(enderman) - level.game_time;
    assert!((400..=780).contains(&remaining), "{remaining}");
    assert!(level.is_angry(enderman) && level.is_angry_at(enderman, player));
    level.player_died(enderman, player);
    assert!(!level.is_angry(enderman));
    assert_eq!((level.get_target(enderman), level.get_persistent_anger_end_time(enderman)), (None, -1));
    assert!(!enderman::is_creepy(level.entity(enderman)));
}

#[test]
fn endermen_dodge_projectiles_by_teleporting() {
    let mut level = level_with_bystander();
    let mut mob = Enderman::create();
    mob.entity.random = LegacyRandomSource::new(1);
    let start = Vec3::new(0.5, -60.0, 0.5);
    let enderman = add(&mut level, mob, start);
    assert!(!level.hurt_server(enderman, DamageSource::new(damage_types::ARROW), 5.0));
    assert_eq!(level.health(enderman), 40.0);
    assert_ne!(level.entity(enderman).position(), start);
    assert!(!level.projectile_receives_side_effects_on_hit(enderman, false));
}

#[test]
fn endermen_are_hurt_by_water() {
    let mut level = level_with_bystander();
    level.set_block(BlockPos::new(0, -60, 0), blocks::WATER.default_state(), UPDATE_ALL);
    let mut mob = Enderman::create();
    mob.set_no_ai(true);
    mob.entity.random = LegacyRandomSource::new(2);
    let enderman = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    level.tick();
    assert_eq!(level.health(enderman), 39.0);
}

#[test]
fn endermen_take_holdable_blocks_and_place_them_back() {
    let mut level = level_at(18000);
    let (from, to) = (BlockPos::new(-1, -60, -1), BlockPos::new(1, -60, 1));
    for pos in BlockPos::between_closed(from, to) {
        if pos != BlockPos::new(0, -60, 0) {
            level.set_block(pos, blocks::GRASS_BLOCK.default_state(), UPDATE_ALL);
        }
    }
    let mut mob = Enderman::create();
    mob.entity.random = LegacyRandomSource::new(3);
    let enderman = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    assert!(EndermanLeaveBlockGoal.can_use(&mut level, enderman) == false);
    let mut ticks = 0;
    while enderman::get_carried_block(level.entity(enderman)).is_none() {
        EndermanTakeBlockGoal.tick(&mut level, enderman);
        ticks += 1;
        assert!(ticks < 1000, "the enderman never took a block");
    }
    assert_eq!(enderman::get_carried_block(level.entity(enderman)), Some(blocks::GRASS_BLOCK.default_state()));
    assert_eq!(count_blocks(&mut level, blocks::GRASS_BLOCK, from, to), 7);
    assert!(level.requires_custom_persistence(enderman));
    while enderman::get_carried_block(level.entity(enderman)).is_some() {
        EndermanLeaveBlockGoal.tick(&mut level, enderman);
        ticks += 1;
        assert!(ticks < 5000, "the enderman never placed its block");
    }
    assert_eq!(count_blocks(&mut level, blocks::GRASS_BLOCK, BlockPos::new(-1, -60, -1), BlockPos::new(1, -59, 1)), 8);
    level.game_rules.set(game_rule::MOB_GRIEFING, 0);
    assert!(!EndermanTakeBlockGoal.can_use(&mut level, enderman));
}

#[test]
fn endermen_save_their_carried_block_and_anger() {
    let mut level = level_at(18000);
    let enderman = add(&mut level, Enderman::create(), Vec3::new(0.5, -60.0, 0.5));
    enderman::set_carried_block(level.entity_mut(enderman), Some(blocks::OAK_LOG.default_state()));
    level.set_persistent_anger_end_time(enderman, 5000);
    level.set_persistent_anger_target(enderman, Some(EntityReference::of(7)));
    let nbt = level.save_entity(enderman).expect("a saved enderman");
    assert_eq!(nbt.get("carriedBlockState"), Some(&Nbt::String("minecraft:oak_log".into())));
    assert_eq!(nbt.get("anger_end_time"), Some(&Nbt::Long(5000)));
    assert!(nbt.get("angry_at").is_some());
    let LevelEntity::Mob(loaded) = level.load_entity(&nbt).expect("a loaded enderman") else { unreachable!() };
    assert_eq!(enderman::get_carried_block(&loaded.entity), Some(blocks::OAK_LOG.default_state()));
    let fresh = add(&mut level, Enderman::create(), Vec3::new(4.5, -60.0, 0.5));
    let fresh = level.save_entity(fresh).expect("a saved enderman");
    assert_eq!(fresh.get("carriedBlockState"), None);
    assert_eq!(fresh.get("anger_end_time"), Some(&Nbt::Long(0)));
    assert_eq!(fresh.get("angry_at"), None);
}

#[test]
fn endermen_drop_their_carried_block_with_silk_touch() {
    let mut level = level_with_bystander();
    let enderman = add(&mut level, Enderman::create(), Vec3::new(0.5, -60.0, 0.5));
    enderman::set_carried_block(level.entity_mut(enderman), Some(blocks::GRASS_BLOCK.default_state()));
    level.kill(enderman);
    let area = AABB::new(-5.0, -65.0, -5.0, 5.0, -50.0, 5.0);
    let dropped: Vec<_> = level.items_in(&area, None).into_iter().map(|item| level.entities[&item].as_item().expect("an item").item.get_item()).collect();
    assert!(dropped.contains(&items::GRASS_BLOCK), "{dropped:?}");
}
