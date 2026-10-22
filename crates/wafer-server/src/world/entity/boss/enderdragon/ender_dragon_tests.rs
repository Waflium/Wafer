use wafer_core::Vec3;
use wafer_data::damage_types;
use wafer_protocol::nbt::Nbt;

use super::ender_dragon::{EnderDragon, ender_dragon};
use super::phases::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::entity::{EntityId, LevelEntity};

fn summon_dragon(level: &mut ServerLevel) -> (EntityId, EntityId) {
    let mut mob = EnderDragon::create();
    mob.entity.snap_to(0.5, -40.0, 0.5, 0.0, 0.0);
    let (id, head) = (mob.entity.id, ender_dragon(&mob).head());
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    (id, head)
}

#[test]
fn dragon_parts_follow_the_dragon_ids_and_take_damage_for_it() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let (id, head) = summon_dragon(&mut level);
    assert_eq!(head, id + 1);
    level.tick();
    assert_eq!(level.dragon_current_phase(id), EnderDragonPhase::Hovering);
    assert_eq!(level.dragon_part_parent(head), Some(id));
    let head_box = level.entity(head).bounding_box();
    assert!(level.get_entities(None, &head_box).contains(&head));
    assert!(!level.get_entities(Some(id), &head_box).contains(&head));
    let source = DamageSource { causing_entity: Some(player), direct_entity: Some(player), ..DamageSource::new(damage_types::PLAYER_ATTACK) };
    assert!(level.hurt_server(head, source, 10.0));
    assert_eq!(level.health(id), 190.0);
}

#[test]
fn dragons_save_their_phase_and_vanish_when_killed() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let (id, head) = summon_dragon(&mut level);
    level.tick();
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    assert!(out.iter().any(|(k, v)| k == "DragonPhase" && *v == Nbt::Int(10)));
    assert!(out.iter().any(|(k, v)| k == "DragonDeathTime" && *v == Nbt::Int(0)));
    assert!(out.iter().any(|(k, v)| k == "sitting_damage_received" && *v == Nbt::Float(0.0)));
    level.kill(id);
    assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()));
    level.tick();
    assert!(level.dragon_part_parent(head).is_none());
}
