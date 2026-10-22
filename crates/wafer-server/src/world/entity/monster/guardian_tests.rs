use wafer_core::Vec3;
use wafer_data::{damage_types, mob_effects};

use super::elder_guardian::ElderGuardian;
use super::guardian::{self, Guardian};
use crate::server::level::game_mode::GameType;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::goal::goal::{Flag, Goal};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::Mob;
use crate::world::entity::{EntityId, LevelEntity};

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

#[test]
fn guardians_have_vanillas_attributes_and_goals() {
    let mob = Guardian::create();
    let attributes = &mob.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 30.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.5);
    assert_eq!(attributes.value(Attribute::AttackDamage), 6.0);
    assert_eq!(mob.xp_reward, 10);
    assert!(mob.is_monster());
    let goals = mob.ai.goal_selector.get_available_goals();
    let priorities: Vec<i32> = goals.iter().map(|wrapped| wrapped.get_priority()).collect();
    assert_eq!(priorities, vec![4, 5, 7, 8, 8, 9]);
    assert_eq!(goals[1].get_flags(), Flag::Move.bit() | Flag::Look.bit());
    assert_eq!(goals[2].get_flags(), Flag::Move.bit() | Flag::Look.bit());
    assert!(matches!(goals[0].get_goal(), Goal::GuardianAttack(_)));
    assert_eq!(mob.client_tracking_range(), 8);
    assert_eq!(mob.get_max_head_x_rot(), 180);
    let elder = ElderGuardian::create();
    let attributes = &elder.living.attributes;
    assert_eq!(attributes.value(Attribute::MaxHealth), 80.0);
    assert_eq!(attributes.value(Attribute::MovementSpeed), 0.3f32 as f64);
    assert_eq!(attributes.value(Attribute::AttackDamage), 8.0);
    assert!(elder.persistence_required);
    assert_eq!(elder.client_tracking_range(), 10);
}

#[test]
fn still_guardians_hurt_their_attackers_with_thorns() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).movement.client_loaded_timeout_timer = 0;
    level.player_mut(player).game_mode.game_type = GameType::Survival;
    let guardian = add(&mut level, Guardian::create(), Vec3::new(2.5, -60.0, 0.5));
    assert!(!guardian::is_moving(level.entity(guardian)));
    let source = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
    level.hurt_server(guardian, source, 1.0);
    assert_eq!(level.health(player), 18.0);
}

#[test]
fn elder_guardians_give_survival_players_mining_fatigue() {
    let mut level = testing::level();
    let near = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(near).game_mode.game_type = GameType::Survival;
    let elder = add(&mut level, ElderGuardian::create(), Vec3::new(10.5, -60.0, 0.5));
    for _ in 0..1201 {
        level.tick();
    }
    let effect = level.player(near).living.effect(mob_effects::MINING_FATIGUE).expect("mining fatigue");
    assert_eq!(effect.amplifier, 2);
    assert!(effect.duration > 6000 - 1201);
    assert!(level.mob(elder).unwrap().has_home());
}
