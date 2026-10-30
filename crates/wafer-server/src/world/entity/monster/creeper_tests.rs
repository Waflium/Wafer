use wafer_core::{BlockPos, Vec3};
use wafer_data::{damage_types, items, mob_effects, registry, registry_id};
use wafer_protocol::nbt::Nbt;

use super::creeper::{self, Creeper};
use super::zombie::zombie::Zombie;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::Mob;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn items_on_ground(level: &ServerLevel) -> Vec<wafer_data::Item> {
    level.entities.values().filter_map(|e| e.as_item()).map(|item| item.item.get_item()).collect()
}

fn loaded_player(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let player = testing::player(level, pos, 0.0, 0.0);
    for _ in 0..61 {
        level.tick();
    }
    player
}

fn swell(level: &ServerLevel, id: EntityId) -> i32 {
    creeper::creeper(level.mob(id).expect("a creeper")).swell
}

#[test]
fn creepers_have_vanillas_attributes() {
    let mob = Creeper::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 20.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 16.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.25);
    assert_eq!(attributes.value(Attribute::AttackDamage), 2.0);
    assert_eq!(mob.xp_reward, 5);
    assert_eq!(mob.entity.dimensions.height, 1.7);
    assert_eq!(mob.entity.dimensions.eye_height, 1.7 * 0.85);
    assert_eq!(creeper::get_swell_dir(&mob.entity), -1);
    assert_eq!(mob.ai.goal_selector.get_available_goals().len(), 8);
    assert_eq!(mob.ai.target_selector.get_available_goals().len(), 2);
}

#[test]
fn creepers_save_and_load_their_fields() {
    let mut mob = Creeper::create();
    let mut out = Vec::new();
    mob.save(&mut out);
    let get = |out: &[(String, Nbt)], key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get(&out, "powered"), Some(Nbt::Byte(0)));
    assert_eq!(get(&out, "Fuse"), Some(Nbt::Short(30)));
    assert_eq!(get(&out, "ExplosionRadius"), Some(Nbt::Byte(3)));
    assert_eq!(get(&out, "ignited"), Some(Nbt::Byte(0)));
    mob.load(&Nbt::Compound(vec![
        ("powered".into(), Nbt::Byte(1)),
        ("Fuse".into(), Nbt::Short(12)),
        ("ExplosionRadius".into(), Nbt::Byte(5)),
        ("ignited".into(), Nbt::Byte(1)),
    ].into()));
    assert!(creeper::is_powered(&mob.entity));
    assert!(creeper::is_ignited(&mob.entity));
    let mut out = Vec::new();
    mob.save(&mut out);
    assert_eq!(get(&out, "powered"), Some(Nbt::Byte(1)));
    assert_eq!(get(&out, "Fuse"), Some(Nbt::Short(12)));
    assert_eq!(get(&out, "ExplosionRadius"), Some(Nbt::Byte(5)));
    assert_eq!(get(&out, "ignited"), Some(Nbt::Byte(1)));
    mob.load(&Nbt::Compound(vec![("ignited".into(), Nbt::Byte(0))].into()));
    assert!(creeper::is_ignited(&mob.entity));
    assert!(!creeper::is_powered(&mob.entity));
}

#[test]
fn creepers_swell_explode_and_hurt_the_player() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = loaded_player(&mut level, Vec3::new(0.5, -60.0, 0.5));
    let id = add(&mut level, Creeper::create(), Vec3::new(2.5, -60.0, 0.5));
    let below = BlockPos::new(2, -61, 0);
    assert!(!level.block_state(below).is_air());
    let mut exploded_at = None;
    for tick in 0..100 {
        level.tick();
        if level.entity_ref(id).is_none() {
            exploded_at = Some(tick);
            break;
        }
    }
    assert!(exploded_at.is_some(), "the creeper never exploded");
    assert!(level.health(player) < 20.0, "the explosion never hurt the player");
    assert!(level.block_state(below).is_air(), "the explosion destroyed no blocks");
}

#[test]
fn hurt_creepers_chase_swell_and_explode_like_vanilla() {
    let expected: Vec<&str> = include_str!("creeper_chase_oracle.txt").lines().collect();
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    level.game_rules.set(wafer_data::game_rule::NATURAL_HEALTH_REGENERATION, 0);
    let player = loaded_player(&mut level, Vec3::new(0.5, -60.0, 0.5));
    level.game_time = 1000;
    let mut mob = Creeper::create();
    while mob.entity.id % 2 != 0 {
        mob = Creeper::create();
    }
    mob.entity.random = wafer_core::random::LegacyRandomSource::new(42);
    let id = add(&mut level, mob, Vec3::new(8.5, -60.0, 0.5));
    let source = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
    level.hurt_server(id, source, 1.0);
    let mut actual = Vec::new();
    for _ in 0..expected.len() {
        level.tick();
        let health = level.health(player);
        match level.entity_ref(id) {
            Some(e) => actual.push(format!("{:?} {:?} {:?} {:?}", e.x(), e.y(), e.z(), health)),
            None => {
                actual.push(format!("gone {health:?}"));
                break;
            }
        }
    }
    for (tick, (actual, expected)) in actual.iter().zip(&expected).enumerate() {
        assert_eq!(actual, expected, "tick {}", tick + 1);
    }
    assert_eq!(actual.len(), expected.len());
}

#[test]
fn creepers_stop_swelling_when_the_player_walks_away() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let id = add(&mut level, Creeper::create(), Vec3::new(2.5, -60.0, 0.5));
    level.set_target(id, Some(player));
    for _ in 0..12 {
        level.tick();
    }
    let swelled = swell(&level, id);
    assert!(swelled > 0, "the creeper never swelled");
    level.entity_mut(player).set_pos(Vec3::new(0.5, -60.0, 20.5));
    for _ in 0..5 {
        level.tick();
    }
    assert_eq!(creeper::get_swell_dir(level.entity(id)), -1);
    assert!(swell(&level, id) < swelled);
}

#[test]
fn creepers_keep_blocks_without_mob_griefing() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    level.game_rules.set(wafer_data::game_rule::MOB_GRIEFING, 0);
    let player = loaded_player(&mut level, Vec3::new(0.5, -60.0, 0.5));
    let id = add(&mut level, Creeper::create(), Vec3::new(2.5, -60.0, 0.5));
    for _ in 0..100 {
        level.tick();
    }
    assert!(level.entity_ref(id).is_none());
    assert!(level.health(player) < 20.0);
    assert!(!level.block_state(BlockPos::new(2, -61, 0)).is_air());
}

#[test]
fn flint_and_steel_ignites_creepers() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::FLINT_AND_STEEL, 1)));
    level.player_mut(player).set_game_mode_for_player(crate::server::level::game_mode::GameType::Creative, None);
    let id = add(&mut level, Creeper::create(), Vec3::new(0.5, -60.0, 2.5));
    let location = level.entity(id).position();
    assert!(level.interact_on(player, id, InteractionHand::MainHand, location).consumes_action());
    assert!(creeper::is_ignited(level.entity(id)));
    let held = level.player(player).item_in_hand(InteractionHand::MainHand).expect("the flint and steel");
    assert_eq!(held.damage_value(), 0);
    for _ in 0..29 {
        level.tick();
    }
    assert!(level.entity_ref(id).is_some());
    level.tick();
    assert!(level.entity_ref(id).is_none());
}

#[test]
fn igniting_damages_flint_and_steel_and_uses_up_fire_charges() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::FLINT_AND_STEEL, 1)));
    let first = add(&mut level, Creeper::create(), Vec3::new(0.5, -60.0, 2.5));
    let location = level.entity(first).position();
    level.interact_on(player, first, InteractionHand::MainHand, location);
    let held = level.player(player).item_in_hand(InteractionHand::MainHand).expect("the flint and steel");
    assert_eq!(held.damage_value(), 1);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::FIRE_CHARGE, 2)));
    let second = add(&mut level, Creeper::create(), Vec3::new(2.5, -60.0, 2.5));
    let location = level.entity(second).position();
    level.interact_on(player, second, InteractionHand::MainHand, location);
    assert!(creeper::is_ignited(level.entity(second)));
    assert_eq!(level.player(player).item_in_hand(InteractionHand::MainHand).map(|stack| stack.count), Some(1));
}

#[test]
fn lightning_charges_creepers() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 30.5), 0.0, 0.0);
    let mut mob = Creeper::create();
    mob.set_no_ai(true);
    let id = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    let bolt = crate::world::entity::lightning_bolt::LightningBolt::new(Vec3::new(0.5, -60.0, 0.5));
    level.add_fresh_entity(LevelEntity::LightningBolt(bolt));
    assert!(!creeper::is_powered(level.entity(id)));
    for _ in 0..3 {
        level.tick();
    }
    assert!(creeper::is_powered(level.entity(id)));
}

#[test]
fn charged_creepers_make_mobs_drop_their_heads() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 30.5), 0.0, 0.0);
    let mut mob = Creeper::create();
    mob.load(&Nbt::Compound(vec![("powered".into(), Nbt::Byte(1)), ("ignited".into(), Nbt::Byte(1)), ("Fuse".into(), Nbt::Short(1))].into()));
    mob.set_no_ai(true);
    add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    let mut zombie = Zombie::create();
    zombie.set_no_ai(true);
    add(&mut level, zombie, Vec3::new(1.5, -60.0, 0.5));
    let mut other = Zombie::create();
    other.set_no_ai(true);
    add(&mut level, other, Vec3::new(-0.5, -60.0, 0.5));
    for _ in 0..3 {
        level.tick();
    }
    let heads = items_on_ground(&level).into_iter().filter(|&item| item == items::ZOMBIE_HEAD).count();
    assert_eq!(heads, 1);
}

#[test]
fn creepers_killed_by_skeletons_drop_music_discs() {
    let mut level = testing::level();
    let id = add(&mut level, Creeper::create(), Vec3::new(0.5, -60.0, 0.5));
    let mut skeleton = Zombie::create();
    skeleton.entity.entity_type = registry_id(registry::ENTITY_TYPE, "minecraft:skeleton");
    skeleton.set_no_ai(true);
    let skeleton = add(&mut level, skeleton, Vec3::new(4.5, -60.0, 0.5));
    let source = DamageSource { causing_entity: Some(skeleton), direct_entity: Some(skeleton), ..DamageSource::new(damage_types::MOB_ATTACK) };
    level.hurt_server(id, source, 100.0);
    let discs = items_on_ground(&level).into_iter().filter(|item| item.is(&wafer_data::item_tags::CREEPER_DROP_MUSIC_DISCS)).count();
    assert_eq!(discs, 1);
}

#[test]
fn falling_makes_creepers_swell() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 40.5), 0.0, 0.0);
    let id = add(&mut level, Creeper::create(), Vec3::new(0.5, -50.0, 0.5));
    let mut landed = None;
    for _ in 0..40 {
        level.tick();
        if level.entity(id).on_ground {
            landed = Some(swell(&level, id));
            break;
        }
    }
    assert!(landed.is_some_and(|swell| (13..=15).contains(&swell)), "swell on landing: {landed:?}");
    assert!(level.health(id) < 20.0);
}

#[test]
fn creepers_with_effects_leave_a_lingering_cloud() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 30.5), 0.0, 0.0);
    let mut mob = Creeper::create();
    mob.load(&Nbt::Compound(vec![("ignited".into(), Nbt::Byte(1)), ("Fuse".into(), Nbt::Short(1))].into()));
    mob.set_no_ai(true);
    let id = add(&mut level, mob, Vec3::new(0.5, -60.0, 0.5));
    level.add_effect(id, MobEffectInstance::new(mob_effects::SPEED, 600, 0), None);
    for _ in 0..3 {
        level.tick();
    }
    let cloud = level.entities.values().find_map(|e| match e {
        LevelEntity::AreaEffectCloud(cloud) => Some(cloud),
        _ => None,
    });
    let cloud = cloud.expect("a lingering cloud");
    assert_eq!(cloud.duration, 300);
    assert_eq!(cloud.wait_time, 10);
    assert!(cloud.get_radius() <= 2.5);
}
