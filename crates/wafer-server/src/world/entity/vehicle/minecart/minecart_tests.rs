use wafer_core::{BlockPos, ChunkPos, Direction, Vec3};
use wafer_data::blocks;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;

fn server() -> Server {
    let mut server = testing::server();
    for x in -2..=3 {
        for z in -2..=2 {
            server.levels[0].chunks.chunk(ChunkPos::new(x, z));
        }
    }
    run(&mut server, "forceload add -16 -16 63 31");
    server.levels[0].finish_chunk_work();
    server
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

fn entity_with_uuid(server: &Server, last: i32) -> Option<EntityId> {
    let uuid = (1u128 << 96) | (2u128 << 64) | (3u128 << 32) | last as u32 as u128;
    server.levels[0].entity_manager.visible_entity_storage.get(uuid)
}

#[test]
fn container_minecarts_save_like_vanilla() {
    let mut server = server();
    run(&mut server, "summon hopper_minecart 4.5 -60 4.5 {UUID:[I;1,2,3,3]}");
    run(&mut server, "summon chest_minecart 2.5 -60 4.5 {UUID:[I;1,2,3,6],LootTable:\"minecraft:chests/abandoned_mineshaft\",LootTableSeed:5L}");
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000003");
    assert!(out[0].contains("Enabled: 1b") && out[0].contains("Items: []"), "{out:?}");
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000006");
    assert!(out[0].contains("LootTable: \"minecraft:chests/abandoned_mineshaft\"") && out[0].contains("LootTableSeed: 5L") && !out[0].contains("Items"), "{out:?}");
    let out = run(&mut server, "item replace entity 00000001-0000-0002-0000-000300000003 container.3 with diamond 5");
    assert_eq!(out, vec!["Replaced 1 slot(s) on Minecart with Hopper with [Diamond]".to_string()]);
}

#[test]
fn hopper_minecarts_suck_up_items_and_hoppers_empty_them() {
    let mut server = server();
    run(&mut server, "setblock 0 -60 6 rail");
    run(&mut server, "summon hopper_minecart 0.5 -60 6.5 {UUID:[I;1,2,3,3]}");
    run(&mut server, "summon item 0.5 -59 6.5 {Item:{id:\"minecraft:diamond\",count:3},PickupDelay:10s}");
    run(&mut server, "setblock 6 -61 6 hopper[facing=down]");
    run(&mut server, "setblock 6 -60 6 rail");
    run(&mut server, "summon hopper_minecart 6.5 -60 6.5 {UUID:[I;1,2,3,7],Items:[{Slot:0b,id:\"minecraft:gold_ingot\",count:4}]}");
    for _ in 0..40 {
        server.tick();
    }
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000003 Items");
    assert_eq!(out, vec!["Minecart with Hopper has the following entity data: [{count: 3, Slot: 0b, id: \"minecraft:diamond\"}]".to_string()]);
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000007 Items");
    assert_eq!(out, vec!["Minecart with Hopper has the following entity data: []".to_string()]);
    let out = run(&mut server, "data get block 6 -61 6 Items");
    assert_eq!(out, vec!["6, -61, 6 has the following block data: [{count: 4, Slot: 0b, id: \"minecraft:gold_ingot\"}]".to_string()]);
}

#[test]
fn furnace_minecarts_push_along_rails_like_vanilla() {
    let mut server = server();
    run(&mut server, "fill 15 -60 2 40 -60 2 rail");
    run(&mut server, "summon furnace_minecart 15.5 -60 2.5 {UUID:[I;1,2,3,2],Fuel:3600s,PushX:1.0d}");
    for _ in 0..40 {
        server.tick();
    }
    let cart = entity_with_uuid(&server, 2).unwrap();
    let e = server.levels[0].entity(cart);
    assert_eq!(e.position(), Vec3::new(23.299999999999972, -59.9375, 2.5));
    assert_eq!(e.delta_movement, Vec3::new(2.495808057580807, 0.0, 0.0));
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000002 Fuel");
    assert_eq!(out, vec!["Minecart with Furnace has the following entity data: 3560s".to_string()]);
}

#[test]
fn powered_activator_rails_prime_tnt_minecarts_that_spare_rails() {
    let mut server = server();
    run(&mut server, "setblock 12 -61 6 redstone_block");
    run(&mut server, "setblock 12 -60 6 activator_rail");
    run(&mut server, "summon tnt_minecart 12.5 -60 6.5 {UUID:[I;1,2,3,4]}");
    for _ in 0..40 {
        server.tick();
    }
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000004 fuse");
    assert_eq!(out, vec!["Minecart with TNT has the following entity data: 40".to_string()]);
    for _ in 0..60 {
        server.tick();
    }
    assert!(entity_with_uuid(&server, 4).is_none());
    let level = &mut server.levels[0];
    assert!(level.block_state(BlockPos::new(12, -61, 6)).is_block(blocks::REDSTONE_BLOCK));
    assert!(level.block_state(BlockPos::new(12, -60, 6)).is_block(blocks::ACTIVATOR_RAIL));
    assert!(level.block_state(BlockPos::new(13, -61, 6)).is_air());
}

#[test]
fn detector_rails_read_container_minecarts() {
    let mut server = server();
    run(&mut server, "setblock 3 -60 3 detector_rail");
    run(&mut server, "summon chest_minecart 3.5 -60 3.5 {Items:[{Slot:0b,id:\"minecraft:stone\",count:64},{Slot:1b,id:\"minecraft:stone\",count:64}]}");
    for _ in 0..2 {
        server.tick();
    }
    let level = &mut server.levels[0];
    let pos = BlockPos::new(3, -60, 3);
    let state = level.block_state(pos);
    let signal = crate::world::level::block::of(state).get_analog_output_signal(state, level, pos, Direction::North);
    assert_eq!(signal, 1 + (2.0f32 / 27.0 * 14.0) as i32);
}

