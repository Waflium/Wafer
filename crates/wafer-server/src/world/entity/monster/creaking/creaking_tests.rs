use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;
use wafer_data::properties::{AXIS, CREAKING_HEART_STATE, CreakingHeartState};

use crate::server::level::{ServerLevel, testing};
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_ALL;

fn add_creaking(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let mut entity = ServerLevel::create_entity("minecraft:creaking").expect("a creaking");
    entity.entity_mut().set_pos(pos);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

#[test]
fn creakings_freeze_while_watched() {
    let mut level = testing::level();
    level.difficulty = crate::world::difficulty::Difficulty::Normal;
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 6.5), 180.0, 0.0);
    level.entity_mut(player).y_head_rot = 180.0;
    let id = add_creaking(&mut level, Vec3::new(0.5, -60.0, 0.5));
    for _ in 0..40 {
        level.tick();
    }
    assert!(!level.creaking_can_move(id));
    level.entity_mut(player).set_y_rot(0.0);
    level.entity_mut(player).y_head_rot = 0.0;
    for _ in 0..40 {
        level.tick();
    }
    assert!(level.creaking_can_move(id));
}

#[test]
fn creaking_hearts_wake_between_pale_oak_logs() {
    let mut level = testing::level();
    let pos = BlockPos::new(0, -58, 0);
    level.set_block(pos, blocks::CREAKING_HEART.default_state().set(AXIS, wafer_core::Axis::Y), UPDATE_ALL);
    level.set_block(pos.below(), blocks::PALE_OAK_LOG.default_state(), UPDATE_ALL);
    level.set_block(pos.above(), blocks::PALE_OAK_LOG.default_state(), UPDATE_ALL);
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 6.5), 180.0, 0.0);
    for _ in 0..5 {
        level.tick();
    }
    assert_ne!(level.block_state(pos).get(CREAKING_HEART_STATE), CreakingHeartState::Uprooted);
    level.set_block(pos.above(), blocks::AIR.default_state(), UPDATE_ALL);
    for _ in 0..30 {
        level.tick();
    }
    assert_eq!(level.block_state(pos).get(CREAKING_HEART_STATE), CreakingHeartState::Uprooted);
}
