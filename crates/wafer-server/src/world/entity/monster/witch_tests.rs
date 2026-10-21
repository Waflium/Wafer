use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, damage_types, items, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::witch::{self, Witch};
use super::creeper::Creeper;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::effect::MobEffectInstance;
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::mob::Mob;
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::component as dc;

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn goal_names(goals: &[crate::world::entity::ai::goal::wrapped_goal::WrappedGoal]) -> Vec<(i32, &'static str)> {
    goals
        .iter()
        .map(|wrapped| {
            let name = match wrapped.get_goal() {
                Goal::LongDistancePatrol(_) => "long_distance_patrol",
                Goal::ObtainRaidLeaderBanner(_) => "obtain_raid_leader_banner",
                Goal::PathfindToRaid(_) => "pathfind_to_raid",
                Goal::RaiderMoveThroughVillage(_) => "raider_move_through_village",
                Goal::RaiderCelebration(_) => "raider_celebration",
                Goal::Float(_) => "float",
                Goal::RangedAttack(_) => "ranged_attack",
                Goal::WaterAvoidingRandomStroll(_) => "stroll",
                Goal::LookAtPlayer(_) => "look_at_player",
                Goal::RandomLookAround(_) => "random_look_around",
                Goal::HurtByTarget(_) => "hurt_by_target",
                Goal::NearestHealableRaiderTarget(_) => "nearest_healable_raider_target",
                Goal::NearestAttackableWitchTarget(_) => "nearest_attackable_witch_target",
                _ => "other",
            };
            (wrapped.get_priority(), name)
        })
        .collect()
}

fn thrown_potion(level: &ServerLevel) -> Option<String> {
    level.entities.values().find_map(|e| match e {
        LevelEntity::Projectile(p) if matches!(p.kind, ProjectileKind::ThrownSplashPotion(_)) => {
            p.get_item().get(dc::POTION_CONTENTS).and_then(|c| c.get("potion")).and_then(Nbt::as_str).map(str::to_string)
        }
        _ => None,
    })
}

#[test]
fn witches_have_vanillas_attributes_and_goals() {
    let mob = Witch::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 26.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.25);
    assert_eq!(attributes.value(Attribute::FollowRange), 16.0);
    assert_eq!(mob.xp_reward, 5);
    assert_eq!(mob.entity.dimensions.height, 1.95);
    assert_eq!(mob.entity.dimensions.eye_height, 1.62);
    assert_eq!(
        goal_names(mob.ai.goal_selector.get_available_goals()),
        vec![
            (4, "long_distance_patrol"),
            (1, "obtain_raid_leader_banner"),
            (3, "pathfind_to_raid"),
            (4, "raider_move_through_village"),
            (5, "raider_celebration"),
            (1, "float"),
            (2, "ranged_attack"),
            (2, "stroll"),
            (3, "look_at_player"),
            (3, "random_look_around"),
        ]
    );
    assert_eq!(
        goal_names(mob.ai.target_selector.get_available_goals()),
        vec![(1, "hurt_by_target"), (2, "nearest_healable_raider_target"), (3, "nearest_attackable_witch_target")]
    );
}

#[test]
fn magic_damage_is_reduced_to_fifteen_percent() {
    let mut level = testing::level();
    let witch = add(&mut level, Witch::create(), Vec3::new(0.5, -60.0, 0.5));
    let creeper = add(&mut level, Creeper::create(), Vec3::new(4.5, -60.0, 0.5));
    let magic = level.damage_after_magic_absorb(witch, &DamageSource::new(damage_types::MAGIC), 10.0);
    assert_eq!(magic, 10.0 * 0.15);
    let own = level.damage_after_magic_absorb(witch, &DamageSource::entity(damage_types::INDIRECT_MAGIC, witch), 10.0);
    assert_eq!(own, 0.0);
    let attack = level.damage_after_magic_absorb(witch, &DamageSource::entity(damage_types::MOB_ATTACK, creeper), 10.0);
    assert_eq!(attack, 10.0);
    assert_eq!(level.damage_after_magic_absorb(creeper, &DamageSource::new(damage_types::MAGIC), 10.0), 10.0);
}

#[test]
fn witches_pick_splash_potions_like_vanilla() {
    let mut level = testing::level();
    let witch = add(&mut level, Witch::create(), Vec3::new(0.5, -60.0, 0.5));
    let target = add(&mut level, Creeper::create(), Vec3::new(9.5, -60.0, 0.5));
    level.witch_perform_ranged_attack(witch, target, 1.0);
    assert_eq!(thrown_potion(&level).as_deref(), Some("minecraft:slowness"));
    let throw = |level: &mut ServerLevel| {
        let potions: Vec<EntityId> = level.entities.iter().filter(|(_, e)| matches!(e, LevelEntity::Projectile(_))).map(|(&id, _)| id).collect();
        for potion in potions {
            level.entities.shift_remove(&potion);
        }
        level.witch_perform_ranged_attack(witch, target, 1.0);
        thrown_potion(level)
    };
    level.add_effect(target, MobEffectInstance::new(mob_effects::SLOWNESS, 200, 0), None);
    assert_eq!(throw(&mut level).as_deref(), Some("minecraft:poison"));
    level.add_effect(target, MobEffectInstance::new(mob_effects::POISON, 200, 0), None);
    assert_eq!(throw(&mut level).as_deref(), Some("minecraft:harming"));
    level.mob_mut(target).unwrap().set_health(7.0);
    level.living_mut(target).active_effects.clear();
    level.add_effect(target, MobEffectInstance::new(mob_effects::SLOWNESS, 200, 0), None);
    assert_eq!(throw(&mut level).as_deref(), Some("minecraft:harming"));
    witch::Witch::set_using_item(level.mob_mut(witch).unwrap(), true);
    assert_eq!(throw(&mut level), None);
}

#[test]
fn witches_heal_raiders_and_drop_them_as_targets() {
    let mut level = testing::level();
    let witch = add(&mut level, Witch::create(), Vec3::new(0.5, -60.0, 0.5));
    let other = add(&mut level, Witch::create(), Vec3::new(5.5, -60.0, 0.5));
    level.mob_mut(witch).unwrap().target = Some(other);
    level.witch_perform_ranged_attack(witch, other, 1.0);
    assert_eq!(thrown_potion(&level).as_deref(), Some("minecraft:regeneration"));
    assert_eq!(level.mob(witch).unwrap().target, None);
}

#[test]
fn drinking_slows_and_applies_the_potion() {
    let mut level = testing::level();
    let witch = add(&mut level, Witch::create(), Vec3::new(0.5, -60.0, 0.5));
    level.living_set_item_slot(witch, EquipmentSlot::Mainhand, witch::create_item_stack(items::POTION, "minecraft:fire_resistance"));
    Witch::set_using_item(level.mob_mut(witch).unwrap(), true);
    level.witch_ai_step(witch);
    assert!(level.item_by_slot(witch, EquipmentSlot::Mainhand).is_none());
    assert!(!Witch::is_drinking_potion(level.mob(witch).unwrap()));
    assert!(level.has_effect(witch, mob_effects::FIRE_RESISTANCE));
    assert_eq!(level.attribute_value(witch, Attribute::MovementSpeed), 0.25);
}

#[test]
fn raider_fields_save_and_load() {
    let mut level = testing::level();
    let mut tag = vec![("id".to_string(), Nbt::String("minecraft:witch".into()))];
    tag.extend([
        ("Patrolling".to_string(), Nbt::Byte(1)),
        ("PatrolLeader".to_string(), Nbt::Byte(1)),
        ("Wave".to_string(), Nbt::Int(3)),
        ("CanJoinRaid".to_string(), Nbt::Byte(1)),
        ("patrol_target".to_string(), Nbt::IntArray(vec![100, -60, 7])),
    ]);
    let LevelEntity::Mob(loaded) = level.load_entity(&Nbt::Compound(tag)).unwrap() else { unreachable!() };
    let raider = loaded.raider().unwrap();
    assert!(raider.can_join_raid());
    assert_eq!(raider.get_wave(), 3);
    assert!(raider.patrolling_monster.is_patrolling());
    assert!(raider.patrolling_monster.is_patrol_leader());
    assert_eq!(raider.patrolling_monster.get_patrol_target(), Some(BlockPos::new(100, -60, 7)));
    let mut out = Vec::new();
    loaded.save(&mut out);
    let keys: Vec<&str> = out.iter().map(|(k, _)| k.as_str()).skip_while(|&k| k != "patrol_target").collect();
    assert_eq!(keys, vec!["patrol_target", "PatrolLeader", "Patrolling", "Wave", "CanJoinRaid"]);
    assert!(!loaded.remove_when_far_away(100.0));
    assert!(loaded.remove_when_far_away(16385.0));
}

#[test]
fn only_naturally_spawned_witches_stay_out_of_raids() {
    let mut level = testing::level();
    let difficulty = level.current_difficulty_at(BlockPos::ZERO);
    for (reason, can_join_raid) in [(EntitySpawnReason::Natural, false), (EntitySpawnReason::Structure, true), (EntitySpawnReason::Command, true)] {
        let (entity, _) = level.finalize_spawn_detached(LevelEntity::Mob(Box::new(Witch::create())), &difficulty, reason, None);
        let LevelEntity::Mob(mob) = entity else { unreachable!() };
        assert_eq!(mob.raider().unwrap().can_join_raid(), can_join_raid);
        assert!(!mob.raider().unwrap().patrolling_monster.is_patrol_leader());
        assert!(mob.living.attributes.has_modifier(Attribute::FollowRange, "minecraft:random_spawn_bonus"));
    }
}

#[test]
fn witches_throw_slowness_then_poison_every_sixty_ticks() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).movement.client_loaded_timeout_timer = 0;
    let witch = add(&mut level, Witch::create(), Vec3::new(9.5, -60.0, 0.5));
    level.set_target(witch, Some(player));
    let mut seen = Vec::new();
    let mut thrown = Vec::new();
    for tick in 1..=130 {
        level.tick();
        for (&id, entity) in &level.entities {
            if let LevelEntity::Projectile(p) = entity
                && !seen.contains(&id)
            {
                seen.push(id);
                let potion = p.get_item().get(dc::POTION_CONTENTS).and_then(|c| c.get("potion")).and_then(Nbt::as_str).map(str::to_string);
                thrown.push((tick, potion.unwrap_or_default()));
            }
        }
    }
    assert_eq!(thrown.iter().map(|(_, name)| name.as_str()).collect::<Vec<_>>(), vec!["minecraft:slowness", "minecraft:poison"]);
    assert!((60..=63).contains(&thrown[0].0), "first throw after {} ticks", thrown[0].0);
    assert_eq!(thrown[1].0 - thrown[0].0, 60);
    assert!(level.has_effect(player, mob_effects::SLOWNESS));
}
