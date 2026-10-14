use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, items, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::husk::Husk;
use super::zombie::{self, Zombie};
use crate::server::level::{ServerLevel, testing};
use crate::world::difficulty::Difficulty;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::Mob;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

#[test]
fn zombies_have_vanillas_attributes() {
    let mob = Zombie::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 20.0);
    assert_eq!(attributes.value(Attribute::FollowRange), 35.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.23f32 as f64);
    assert_eq!(attributes.value(Attribute::AttackDamage), 3.0);
    assert_eq!(attributes.value(Attribute::Armor), 2.0);
    assert_eq!(attributes.value(Attribute::SpawnReinforcements), 0.0);
    assert_eq!(mob.xp_reward, 5);
    assert_eq!(mob.entity.dimensions.height, 1.95);
}

#[test]
fn baby_zombies_are_small_and_fast() {
    let mut mob = Zombie::create();
    mob.set_baby(true);
    assert!(mob.is_baby());
    assert_eq!(mob.entity.dimensions.width, 0.49);
    assert_eq!(mob.entity.dimensions.height, 0.98);
    assert_eq!(mob.living.attributes.value(Attribute::MovementSpeed), 0.23f32 as f64 * 1.5);
    mob.set_baby(false);
    assert_eq!(mob.living.attributes.value(Attribute::MovementSpeed), 0.23f32 as f64);
    assert_eq!(mob.entity.dimensions.height, 1.95);
}

#[test]
fn zombies_save_and_load_their_fields() {
    let mut mob = Zombie::create();
    mob.set_baby(true);
    Zombie::set_can_break_doors(&mut mob, true);
    let mut out = Vec::new();
    mob.save(&mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("IsBaby"), Some(Nbt::Byte(1)));
    assert_eq!(get("CanBreakDoors"), Some(Nbt::Byte(1)));
    assert_eq!(get("DrownedConversionTime"), Some(Nbt::Int(-1)));
    assert_eq!(get("InWaterTime"), Some(Nbt::Int(-1)));
    let mut loaded = Zombie::create();
    loaded.load(&Nbt::Compound(out));
    assert!(loaded.is_baby());
    assert!(Zombie::can_break_doors(&loaded));
    assert_eq!(loaded.ai.goal_selector.get_available_goals().len(), mob.ai.goal_selector.get_available_goals().len());
}

#[test]
fn zombies_chase_and_hit_survival_players() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let zombie = add(&mut level, Zombie::create(), Vec3::new(8.5, -60.0, 0.5));
    for _ in 0..120 {
        level.tick();
    }
    assert_eq!(level.get_target(zombie), Some(player));
    assert!(level.health(player) < 20.0, "the zombie never hit the player");
    for _ in 0..200 {
        level.tick();
    }
    assert!(!level.is_alive(player));
    assert_eq!(level.get_target(zombie), None);
}

#[test]
fn chasing_zombies_walk_and_hit_like_vanilla() {
    let expected: Vec<(Vec3, f32)> = include_str!("chase_oracle.txt")
        .lines()
        .map(|line| {
            let v: Vec<f64> = line.split(' ').map(|n| n.parse().unwrap()).collect();
            (Vec3::new(v[0], v[1], v[2]), v[3] as f32)
        })
        .skip_while(|(pos, _)| *pos == Vec3::new(8.5, -60.0, 0.5))
        .take(65)
        .collect();
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    level.game_time = 1000;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = Zombie::create();
    while mob.entity.id % 2 != 0 {
        mob = Zombie::create();
    }
    let zombie = add(&mut level, mob, Vec3::new(8.5, -60.0, 0.5));
    level.set_target(zombie, Some(player));
    let mut actual = Vec::new();
    for _ in 0..200 {
        level.tick();
        let pos = level.entity(zombie).position();
        if pos != Vec3::new(8.5, -60.0, 0.5) || !actual.is_empty() {
            actual.push((pos, level.health(player)));
        }
        if actual.len() == expected.len() {
            break;
        }
    }
    let positions = |path: &[(Vec3, f32)]| path.iter().map(|(pos, _)| *pos).collect::<Vec<_>>();
    assert_eq!(positions(&actual), positions(&expected));
    let first_hit = |path: &[(Vec3, f32)]| path.iter().position(|(_, health)| *health < 20.0).map(|i| (i, path[i].1));
    assert_eq!(first_hit(&actual), first_hit(&expected));
}

#[test]
fn zombies_ignore_creative_players() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).set_game_mode_for_player(crate::server::level::game_mode::GameType::Creative, None);
    let zombie = add(&mut level, Zombie::create(), Vec3::new(4.5, -60.0, 0.5));
    for _ in 0..100 {
        level.tick();
    }
    assert_eq!(level.get_target(zombie), None);
    assert_eq!(level.health(player), 20.0);
}

#[test]
fn zombies_burn_in_daylight_unless_helmeted() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 20.5), 0.0, 0.0);
    let bare = add(&mut level, Zombie::create(), Vec3::new(0.5, -60.0, 0.5));
    let mut helmeted = Zombie::create();
    helmeted.equipment[EquipmentSlot::Head as usize] = Some(ItemStack::new(items::LEATHER_HELMET, 1));
    let helmeted = add(&mut level, helmeted, Vec3::new(4.5, -60.0, 0.5));
    for _ in 0..300 {
        level.tick();
    }
    assert!(level.entity(bare).remaining_fire_ticks > 0 || !level.is_alive(bare) || level.health(bare) < 20.0, "the bare zombie never burned");
    assert_eq!(level.entity(helmeted).remaining_fire_ticks, 0);
    assert!(level.item_by_slot(helmeted, EquipmentSlot::Head).is_some_and(|helmet| helmet.damage_value() > 0));
}

#[test]
fn husks_do_not_burn_and_inflict_hunger() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let husk = add(&mut level, Husk::create(), Vec3::new(1.5, -60.0, 0.5));
    let mut hit = false;
    for _ in 0..200 {
        level.tick();
        hit |= level.living(player).has_effect(mob_effects::HUNGER);
    }
    assert_eq!(level.entity(husk).remaining_fire_ticks, 0);
    assert!(hit, "the husk never inflicted hunger");
}

#[test]
fn finalize_spawn_sets_vanillas_attribute_modifiers() {
    let mut level = testing::level();
    let mob = Zombie::create();
    let difficulty = level.current_difficulty_at(BlockPos::ZERO);
    let (entity, group_data) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, EntitySpawnReason::Command, None);
    let LevelEntity::Mob(mob) = entity else { unreachable!() };
    assert!(matches!(group_data, Some(crate::world::entity::spawn_group_data::SpawnGroupData::Zombie(_))));
    let attributes = &mob.living.attributes;
    assert!(attributes.has_modifier(Attribute::FollowRange, "minecraft:random_spawn_bonus"));
    assert!(attributes.has_modifier(Attribute::KnockbackResistance, "minecraft:random_spawn_bonus"));
    assert!(attributes.base_value(Attribute::SpawnReinforcements) < 0.1);
}

#[test]
fn hurt_zombies_call_reinforcements_on_hard() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.difficulty = Difficulty::Hard;
    level.sky_darken = 11;
    let mut mob = Zombie::create();
    mob.living.attributes.instance(Attribute::SpawnReinforcements).unwrap().set_base_value(1.0);
    let caller = add(&mut level, mob, Vec3::new(30.5, -60.0, 0.5));
    let source = crate::world::damagesource::DamageSource::entity(wafer_data::damage_types::PLAYER_ATTACK, player);
    assert!(level.hurt_server(caller, source, 1.0));
    let reinforcements: Vec<EntityId> = level.entities.keys().copied().filter(|&id| id != caller && level.is_mob(id)).collect();
    assert_eq!(reinforcements.len(), 1);
    let reinforcement = reinforcements[0];
    assert_eq!(level.get_target(reinforcement), Some(player));
    let caller_attributes = &level.mob(caller).unwrap().living.attributes;
    assert_eq!(caller_attributes.value(Attribute::SpawnReinforcements), 1.0 - 0.05);
    let callee_attributes = &level.mob(reinforcement).unwrap().living.attributes;
    assert!(callee_attributes.value(Attribute::SpawnReinforcements) < 0.1 - 0.05);
    let (from, to) = (level.entity(caller).position(), level.entity(reinforcement).position());
    let offsets = [to.x - from.x.floor(), to.y - from.y.floor(), to.z - from.z.floor()];
    assert!(offsets.iter().all(|&d| d == 0.0 || (7.0..=40.0).contains(&d.abs())), "{offsets:?}");
}

#[test]
fn baby_zombies_drop_experience() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = Zombie::create();
    mob.set_baby(true);
    let baby = add(&mut level, mob, Vec3::new(2.5, -60.0, 0.5));
    level.player_attack(player, baby);
    level.kill(baby);
    let orbs = level.entities.values().filter(|e| matches!(e, LevelEntity::ExperienceOrb(_))).count();
    assert!(orbs > 0);
    assert!(zombie::is_baby(level.entity(baby)));
}
