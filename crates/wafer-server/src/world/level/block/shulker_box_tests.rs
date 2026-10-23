use wafer_core::{Axis, BlockPos, ChunkPos};
use wafer_data::blocks;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::level::BlockGetter;

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
