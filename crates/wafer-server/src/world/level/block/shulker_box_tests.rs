use wafer_core::{Axis, BlockPos, ChunkPos, Vec3};
use wafer_data::blocks;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::entity::vehicle::dismount_helper;
use crate::world::level::BlockGetter;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::walk_node_evaluator;
use crate::world::phys::shapes::collision_context::CollisionContext;

fn server() -> Server {
    let mut server = testing::server();
    for x in -1..=2 {
        for z in -1..=2 {
            server.levels[0].chunks.chunk(ChunkPos::new(x, z));
        }
    }
    let source = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &source, "forceload add 0 0 31 31");
    for _ in 0..2 {
        server.tick();
    }
    server
}

#[test]
fn open_shulker_boxes_use_their_live_shape() {
    let mut server = server();
    let source = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &source, "setblock 0 -50 0 shulker_box[facing=up]");
    let pos = BlockPos::new(0, -50, 0);
    let state = server.levels[0].block_state(pos);
    let level = &mut server.levels[0];
    assert!(level.is_redstone_conductor_at(pos, state));
    assert!(level.is_collision_shape_full_block_at(pos, state));
    assert!(level.is_suffocating_at(pos, state));
    assert_eq!(level.get_state_collision_shape(state, pos).max(Axis::Y), 1.0);
    level.block_event(pos, blocks::SHULKER_BOX, 1, 1);
    for _ in 0..12 {
        server.tick();
    }
    let level = &mut server.levels[0];
    assert!(!level.is_redstone_conductor_at(pos, state));
    assert!(!level.is_collision_shape_full_block_at(pos, state));
    assert!(!level.is_suffocating_at(pos, state));
    assert_eq!(level.get_state_collision_shape(state, pos).max(Axis::Y), 1.5);
    level.block_event(pos, blocks::SHULKER_BOX, 1, 0);
    for _ in 0..12 {
        server.tick();
    }
    let level = &mut server.levels[0];
    assert!(level.is_redstone_conductor_at(pos, state));
    assert_eq!(level.get_state_collision_shape(state, pos).max(Axis::Y), 1.0);
}

#[test]
fn ai_and_clips_see_the_live_shape_of_open_shulker_boxes() {
    let mut server = server();
    let source = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &source, "setblock 0 -50 0 shulker_box[facing=up]");
    let pos = BlockPos::new(0, -50, 0);
    let state = server.levels[0].block_state(pos);
    server.levels[0].block_event(pos, blocks::SHULKER_BOX, 1, 1);
    for _ in 0..12 {
        server.tick();
    }
    let level = &mut server.levels[0];
    assert_eq!(dismount_helper::get_block_floor_height(level, pos), 1.5);
    assert_eq!(dismount_helper::non_climbable_shape(level, pos).max(Axis::Y), 1.5);
    assert_eq!(dismount_helper::find_ceiling_from(level, pos, 2), pos.y as f64);
    assert_eq!(level.get_state_collision_shape_in_context(state, pos, &CollisionContext::EMPTY).max(Axis::Y), 1.5);
    let region = PathNavigationRegion::new(level);
    assert_eq!(region.get_state_collision_shape(state, pos).max(Axis::Y), 1.5);
    assert_eq!(walk_node_evaluator::get_floor_level(&region, pos.above()), pos.y as f64 + 1.5);
    for block in [ClipBlock::Collider, ClipBlock::Visual] {
        let context = ClipContext { from: Vec3::new(0.5, -48.0, 0.5), to: Vec3::new(0.5, -52.0, 0.5), block, fluid: ClipFluid::None, collision_context: CollisionContext::EMPTY };
        let result = level.clip(&context);
        assert!(!result.miss);
        assert!((result.hit.location.y + 48.5).abs() < 1.0e-9, "{result:?}");
    }
}
