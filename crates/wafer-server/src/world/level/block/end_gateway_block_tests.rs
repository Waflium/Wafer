use wafer_core::{AABB, BlockPos, Vec3};
use wafer_data::blocks;

use crate::server::level::{ServerLevel, testing};
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::clip::for_each_block_intersected_between_grouped;
use crate::world::level::update_flags;

fn level_with_gateway(gateway: BlockPos) -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(gateway.x as f64 + 0.5, gateway.y as f64, gateway.z as f64 + 0.5), 0.0, 0.0);
    level.set_block(gateway, blocks::END_GATEWAY.default_state(), update_flags::UPDATE_ALL);
    (level, player)
}

#[test]
fn block_events_outside_the_ticking_range_stay_queued() {
    let mut server = testing::server();
    let far = BlockPos::new(100_000, -50, 100_000);
    server.levels[0].block_event(far, blocks::END_GATEWAY, 1, 0);
    for _ in 0..3 {
        server.tick();
    }
    assert_eq!(server.levels[0].block_event_count(), 1);
    assert_eq!(server.levels[0].block_event_pos(0), far);
}

#[test]
fn block_intersections_report_where_each_group_ends() {
    let aabb = AABB::new(0.2, 0.0, 0.2, 0.8, 1.8, 0.8);
    let mut visited = Vec::new();
    let (completed, cursors) = for_each_block_intersected_between_grouped(Vec3::new(0.5, 0.0, 0.5), Vec3::new(0.5, 0.0, 0.5), &aabb, |pos, iteration, group| {
        visited.push((pos, iteration, group));
        true
    });
    assert!(completed);
    assert_eq!(visited, vec![(BlockPos::new(0, 0, 0), 0, 0), (BlockPos::new(0, 1, 0), 0, 0)]);
    assert_eq!(cursors, vec![BlockPos::new(0, 1, 0)]);
    let (completed, cursors) = for_each_block_intersected_between_grouped(Vec3::new(0.5, 0.0, 0.5), Vec3::new(0.5, 0.0, 0.5), &aabb, |pos, _, _| pos != BlockPos::new(0, 0, 0));
    assert!(!completed);
    assert_eq!(cursors, vec![BlockPos::new(0, 0, 0)]);
}

#[test]
fn queued_gateway_events_follow_the_end_of_the_block_iteration() {
    let gateway = BlockPos::new(0, -50, 0);
    let (mut level, player) = level_with_gateway(gateway);
    level.apply_effects_from_blocks_between(player, Vec3::new(0.5, -50.0, 0.5), Vec3::new(0.5, -50.0, 0.5));
    assert_eq!(level.end_gateway_is_cooling_down(gateway), Some(true));
    assert_eq!(level.block_event_count(), 1);
    assert_eq!(level.block_event_pos(0), BlockPos::new(0, -49, 0));
    assert!(!level.block_state(BlockPos::new(0, -49, 0)).is_block(blocks::END_GATEWAY));
}

#[test]
fn queued_gateway_events_follow_the_end_of_a_moving_iteration() {
    let gateway = BlockPos::new(0, -50, 0);
    let (mut level, player) = level_with_gateway(gateway);
    level.apply_effects_from_blocks_between(player, Vec3::new(0.5, -50.0, 3.5), Vec3::new(0.5, -50.0, 0.5));
    assert_eq!(level.end_gateway_is_cooling_down(gateway), Some(true));
    assert_eq!(level.block_event_count(), 1);
    assert_ne!(level.block_event_pos(0), gateway);
}

#[test]
fn queued_gateway_events_stay_when_the_iteration_ends_on_the_gateway() {
    let gateway = BlockPos::new(0, -50, 0);
    let (mut level, player) = level_with_gateway(gateway);
    level.entity_mut(player).set_pos(Vec3::new(0.5, -50.4, 0.5));
    level.entity_mut(player).dimensions.height = 0.5;
    level.apply_effects_from_blocks_between(player, Vec3::new(0.5, -50.4, 0.5), Vec3::new(0.5, -50.4, 0.5));
    assert_eq!(level.end_gateway_is_cooling_down(gateway), Some(true));
    assert_eq!(level.block_event_count(), 1);
    assert_eq!(level.block_event_pos(0), gateway);
}
