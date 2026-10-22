use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, mob_effects};
use wafer_protocol::nbt::Nbt;

use super::shulker::{self, DATA_PEEK_ID, Shulker};
use crate::server::level::{ServerLevel, testing};
use crate::world::difficulty::Difficulty;
use crate::world::entity::data::DataValue;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::update_flags::UPDATE_ALL;

fn add_shulker(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let mut mob = Shulker::create();
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    mob.persistence_required = true;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

#[test]
fn shulkers_snap_to_block_centres_and_grow_when_peeking() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    let id = add_shulker(&mut level, Vec3::new(2.3, -59.8, 4.9));
    level.mob_mut(id).unwrap().set_no_ai(true);
    let e = level.entity(id);
    assert_eq!(e.position(), Vec3::new(2.5, -60.0, 4.5));
    let bb = e.bounding_box();
    assert_eq!((bb.min_y, bb.max_y, bb.min_x, bb.max_x), (-60.0, -59.0, 2.0, 3.0));
    level.entity_mut(id).data.set(DATA_PEEK_ID, DataValue::Byte(100));
    for _ in 0..20 {
        level.tick();
    }
    let bb = level.entity(id).bounding_box();
    assert_eq!((bb.min_y, bb.max_y), (-60.0, -58.0));
    assert_eq!(level.entity(id).delta_movement, Vec3::ZERO);
}

#[test]
fn shulkers_save_their_attachment_peek_and_colour() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    let id = add_shulker(&mut level, Vec3::new(0.5, -60.0, 0.5));
    let mut input = Vec::new();
    input.push(("AttachFace".to_string(), Nbt::Byte(0)));
    input.push(("Peek".to_string(), Nbt::Byte(30)));
    input.push(("Color".to_string(), Nbt::Byte(4)));
    Shulker::read_additional_save_data(level.mob_mut(id).unwrap(), &Nbt::Compound(input));
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("AttachFace"), Some(Nbt::Byte(0)));
    assert_eq!(get("Peek"), Some(Nbt::Byte(30)));
    assert_eq!(get("Color"), Some(Nbt::Byte(4)));
    assert_eq!(shulker::get_color(level.entity(id)).map(|c| c.name), Some("yellow"));
}

#[test]
fn shulkers_teleport_when_their_support_disappears() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    level.set_block(BlockPos::new(0, -55, 0), blocks::STONE.default_state(), UPDATE_ALL);
    let id = add_shulker(&mut level, Vec3::new(0.5, -54.0, 0.5));
    level.tick();
    assert_eq!(level.entity(id).block_position(), BlockPos::new(0, -54, 0));
    level.set_block(BlockPos::new(0, -55, 0), blocks::AIR.default_state(), UPDATE_ALL);
    for _ in 0..20 {
        level.tick();
    }
    assert_ne!(level.entity(id).block_position(), BlockPos::new(0, -54, 0));
}

#[test]
fn shulker_bullets_levitate_their_target() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(4.5, -60.0, 0.5), 0.0, 0.0);
    let id = add_shulker(&mut level, Vec3::new(0.5, -60.0, 0.5));
    level.set_target(id, Some(player));
    let mut levitated = false;
    for _ in 0..400 {
        level.tick();
        if level.has_effect(player, mob_effects::LEVITATION) {
            levitated = true;
            break;
        }
    }
    assert!(levitated, "no bullet hit the player");
}

#[test]
fn opening_shulker_boxes_push_entities_out() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::SHULKER_BOX.default_state().set(wafer_data::properties::FACING, wafer_core::Direction::Up), UPDATE_ALL);
    let chicken = level.spawn_entity_type("minecraft:chicken", BlockPos::new(0, -59, 0), crate::world::entity::entity_spawn_reason::EntitySpawnReason::Command).unwrap();
    level.entity_mut(chicken).set_pos(Vec3::new(0.5, -59.0, 0.5));
    level.shulker_box_start_open(pos);
    level.tick();
    assert!(level.entity(chicken).y() > -59.0, "the chicken was not pushed: {}", level.entity(chicken).y());
}
