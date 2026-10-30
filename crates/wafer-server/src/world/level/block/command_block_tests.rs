use wafer_core::{BlockPos, ChunkPos};
use wafer_data::blocks;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::level::BlockGetter;

fn server() -> Server {
    let mut server = testing::server();
    for x in -1..=1 {
        for z in -1..=1 {
            server.levels[0].chunks.chunk(ChunkPos::new(x, z));
        }
    }
    run(&mut server, "forceload add 0 0 15 15");
    server.levels[0].finish_chunk_work();
    for _ in 0..2 {
        server.tick();
    }
    server
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

#[test]
fn impulse_command_blocks_run_their_chain_when_powered() {
    let mut server = server();
    run(&mut server, "setblock 0 -60 0 command_block[facing=east]{Command:\"setblock 5 -60 0 stone\"}");
    run(&mut server, "setblock 1 -60 0 chain_command_block[facing=east]{Command:\"setblock 6 -60 0 stone\",auto:1b}");
    run(&mut server, "setblock 2 -60 0 chain_command_block[facing=east,conditional=true]{Command:\"not_a_command\",auto:1b}");
    run(&mut server, "setblock 3 -60 0 chain_command_block[facing=east,conditional=true]{Command:\"setblock 7 -60 0 stone\",auto:1b}");
    run(&mut server, "setblock 0 -61 0 redstone_block");
    for _ in 0..3 {
        server.tick();
    }
    let level = &mut server.levels[0];
    assert!(level.block_state(BlockPos::new(5, -60, 0)).is_block(blocks::STONE));
    assert!(level.block_state(BlockPos::new(6, -60, 0)).is_block(blocks::STONE));
    assert!(level.block_state(BlockPos::new(7, -60, 0)).is_air());
    assert_eq!(level.command_block_entity(BlockPos::new(0, -60, 0)).unwrap().command_block.get_success_count(), 1);
    assert_eq!(level.command_block_entity(BlockPos::new(2, -60, 0)).unwrap().command_block.get_success_count(), 0);
}

#[test]
fn repeating_command_blocks_placed_with_auto_run_every_tick() {
    let mut server = server();
    run(&mut server, "scoreboard objectives add t dummy");
    run(&mut server, "setblock 0 -60 4 repeating_command_block{Command:\"scoreboard players add #c t 1\",auto:1b}");
    for _ in 0..5 {
        server.tick();
    }
    assert_eq!(run(&mut server, "scoreboard players get #c t"), vec!["#c has 5 [t]".to_string()]);
}
