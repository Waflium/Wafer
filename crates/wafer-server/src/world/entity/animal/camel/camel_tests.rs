use wafer_core::Vec3;

use crate::server::level::{ServerLevel, testing};
use crate::world::entity::EntityId;
use crate::world::entity::data::Pose;

fn add_camel(level: &mut ServerLevel, name: &str, pos: Vec3) -> EntityId {
    let mut entity = ServerLevel::create_entity(name).expect("a camel");
    entity.entity_mut().set_pos(pos);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

#[test]
fn camels_sit_down_and_stand_up() {
    let mut level = testing::level();
    let id = add_camel(&mut level, "minecraft:camel", Vec3::new(0.5, -60.0, 0.5));
    level.tick();
    assert!(!level.camel_is_camel_sitting(id));
    level.camel_sit_down(id);
    assert!(level.camel_is_camel_sitting(id));
    assert_eq!(level.entity(id).pose(), Pose::Sitting);
    assert_eq!(level.entity(id).dimensions.height, 2.375 - 1.43);
    assert!(level.camel_refuse_to_move(id));
    level.camel_stand_up(id);
    assert!(!level.camel_is_camel_sitting(id));
    assert!(level.camel_is_in_pose_transition(id));
    assert_eq!(level.entity(id).dimensions.height, 2.375);
}

#[test]
fn camels_save_their_last_pose_tick() {
    let mut level = testing::level();
    let id = add_camel(&mut level, "minecraft:camel", Vec3::new(0.5, -60.0, 0.5));
    level.tick();
    level.camel_sit_down(id);
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("LastPoseTick"), Some(wafer_protocol::nbt::Nbt::Long(-level.game_time)));
    assert_eq!(get("Tame"), Some(wafer_protocol::nbt::Nbt::Byte(1)));
    let mut loaded = ServerLevel::create_entity("minecraft:camel").expect("a camel");
    loaded.load(&wafer_protocol::nbt::Nbt::Compound(out)).expect("loads");
    assert_eq!(loaded.entity().pose(), Pose::Sitting);
    assert_eq!(loaded.entity().dimensions.height, 2.375 - 1.43);
}

#[test]
fn camel_husks_cannot_be_babies() {
    let mut level = testing::level();
    let id = add_camel(&mut level, "minecraft:camel_husk", Vec3::new(0.5, -60.0, 0.5));
    level.mob_mut(id).unwrap().set_baby(true);
    assert!(!level.is_baby(id));
}

#[test]
fn camels_dash_and_cool_down() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add_camel(&mut level, "minecraft:camel", Vec3::new(0.5, -60.0, 0.5));
    for _ in 0..5 {
        level.tick();
    }
    level.camel_handle_start_jump(id);
    assert!(super::camel::is_dashing(level.entity(id)));
    assert_eq!(level.camel_get_jump_cooldown(id), super::camel::DASH_COOLDOWN_TICKS);
    for _ in 0..7 {
        level.tick();
    }
    assert!(!super::camel::is_dashing(level.entity(id)));
    for _ in 0..60 {
        level.tick();
    }
    assert_eq!(level.camel_get_jump_cooldown(id), 0);
}

#[test]
fn natural_husks_sometimes_ride_camel_husks() {
    let mut level = testing::level();
    level.difficulty = crate::world::difficulty::Difficulty::Normal;
    let mut riders = 0;
    for _ in 0..200 {
        let Some(crate::world::entity::LevelEntity::Mob(mut mob)) = ServerLevel::create_entity("minecraft:husk") else { unreachable!() };
        mob.entity.set_pos(Vec3::new(0.5, -60.0, 0.5));
        let difficulty = level.current_difficulty_at(mob.entity.block_position());
        let (entity, _) = level.finalize_spawn_detached(crate::world::entity::LevelEntity::Mob(mob), &difficulty, crate::world::entity::entity_spawn_reason::EntitySpawnReason::Natural, None);
        let husk = entity.entity().id;
        if let Some(vehicle) = entity.entity().vehicle.filter(|&vehicle| level.is_camel_husk(vehicle)) {
            riders += 1;
            let passengers = level.passengers(vehicle).to_vec();
            assert_eq!(passengers.len(), 2);
            assert_eq!(passengers[0], husk);
            assert!(level.mob(passengers[1]).is_some_and(|mob| matches!(mob.kind, crate::world::entity::mob::MobKind::Parched(_))));
            level.add_fresh_entity(entity);
            assert!(level.is_mob_controlled(vehicle));
            continue;
        }
        level.add_fresh_entity(entity);
    }
    assert!(riders > 0);
}
