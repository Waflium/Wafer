use wafer_core::Vec3;
use wafer_protocol::nbt::Nbt;

use super::evoker::Evoker;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::ai::goal::goal::Goal;
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::Mob;
use crate::world::entity::monster::creeper::Creeper;
use crate::world::entity::monster::vex::Vex;
use crate::world::entity::projectile::evoker_fangs::EvokerFangs;
use crate::world::entity::{EntityId, LevelEntity};

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

#[test]
fn evokers_and_vexes_have_vanillas_attributes_and_goals() {
    let evoker = Evoker::create();
    assert_eq!(evoker.living.attributes.value(Attribute::MaxHealth), 24.0);
    assert_eq!(evoker.living.attributes.value(Attribute::MovementSpeed), 0.5);
    assert_eq!(evoker.living.attributes.value(Attribute::FollowRange), 12.0);
    assert_eq!(evoker.xp_reward, 10);
    let priorities: Vec<i32> = evoker.ai.goal_selector.get_available_goals().iter().map(|wrapped| wrapped.get_priority()).collect();
    assert_eq!(priorities, vec![4, 1, 3, 4, 5, 0, 1, 2, 3, 4, 5, 6, 8, 9, 10]);
    assert!(matches!(evoker.ai.goal_selector.get_available_goals()[11].get_goal(), Goal::EvokerWololoSpell(_)));
    let vex = Vex::create();
    assert_eq!(vex.living.attributes.value(Attribute::MaxHealth), 14.0);
    assert_eq!(vex.living.attributes.value(Attribute::AttackDamage), 4.0);
    assert_eq!(vex.xp_reward, 3);
    let priorities: Vec<i32> = vex.ai.target_selector.get_available_goals().iter().map(|wrapped| wrapped.get_priority()).collect();
    assert_eq!(priorities, vec![1, 2, 3]);
}

#[test]
fn evoker_fangs_bite_after_their_warmup_and_vanish() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 0.0, 0.0);
    let creeper = add(&mut level, Creeper::create(), Vec3::new(0.5, -60.0, 0.5));
    level.mob_mut(creeper).unwrap().set_no_ai(true);
    let mut fangs = EvokerFangs::of_type();
    fangs.entity.set_pos(Vec3::new(0.5, -60.0, 0.5));
    let fangs_id = fangs.entity.id;
    level.add_fresh_entity(LevelEntity::EvokerFangs(fangs));
    for _ in 0..7 {
        level.tick();
    }
    assert_eq!(level.health(creeper), 20.0);
    level.tick();
    assert_eq!(level.health(creeper), 14.0);
    for _ in 0..30 {
        level.tick();
    }
    assert!(level.entity_ref(fangs_id).is_none_or(|e| e.is_removed()));
}

#[test]
fn vexes_starve_when_their_life_runs_out() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 0.0, 0.0);
    let mut mob = Vex::create();
    mob.load(&Nbt::Compound(vec![("life_ticks".into(), Nbt::Int(2))]));
    mob.set_no_ai(true);
    let vex = add(&mut level, mob, Vec3::new(0.5, -55.0, 0.5));
    level.tick();
    assert_eq!(level.health(vex), 14.0);
    level.tick();
    assert_eq!(level.health(vex), 13.0);
    assert!(level.entity(vex).is_no_gravity());
}

#[test]
fn illusioners_have_vanillas_attributes_and_goals() {
    let illusioner = super::illusioner::Illusioner::create();
    assert_eq!(illusioner.living.attributes.value(Attribute::MaxHealth), 32.0);
    assert_eq!(illusioner.living.attributes.value(Attribute::FollowRange), 18.0);
    assert_eq!(illusioner.xp_reward, 5);
    let priorities: Vec<i32> = illusioner.ai.goal_selector.get_available_goals().iter().map(|wrapped| wrapped.get_priority()).collect();
    assert_eq!(priorities, vec![4, 1, 3, 4, 5, 0, 1, 3, 4, 5, 6, 8, 9, 10]);
    assert!(matches!(illusioner.ai.goal_selector.get_available_goals()[10].get_goal(), Goal::RangedBowAttack(_)));
}
