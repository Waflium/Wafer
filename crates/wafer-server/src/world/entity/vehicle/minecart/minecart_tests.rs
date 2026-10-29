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

fn activator_line(server: &mut Server, z: i32) {
    run(server, &format!("fill 0 -61 {z} 14 -61 {z} redstone_block"));
    run(server, &format!("fill 0 -60 {z} 14 -60 {z} activator_rail[shape=east_west,powered=true]"));
}

fn tick(server: &mut Server, count: usize) {
    for _ in 0..count {
        server.tick();
    }
}

#[test]
fn command_block_minecarts_run_their_command_before_the_rest_of_their_tick() {
    let mut server = server();
    activator_line(&mut server, 10);
    run(&mut server, "summon command_block_minecart 0.5 -60 10.5 {UUID:[I;1,2,3,5],Command:\"setblock ~ ~ ~ fire\"}");
    tick(&mut server, 3);
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000005 Fire");
    assert_eq!(out, vec!["Minecart with Command Block has the following entity data: 0s".to_string()]);
    tick(&mut server, 1);
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000005 Fire");
    assert_eq!(out, vec!["Minecart with Command Block has the following entity data: 160s".to_string()]);
}

#[test]
fn command_block_minecarts_see_their_state_in_the_middle_of_the_improved_movement() {
    let mut server = server();
    server.levels[0].enabled_features = crate::world::flag::FeatureFlagSet::DEFAULT_FLAGS.join(crate::world::flag::FeatureFlagSet::MINECART_IMPROVEMENTS);
    activator_line(&mut server, 12);
    run(&mut server, "summon command_block_minecart 0.5 -60 12.5 {UUID:[I;1,2,3,5],Motion:[0.3d,0.0d,0.0d],Command:\"data modify storage t:t pos set from entity @s Pos\"}");
    tick(&mut server, 4);
    let out = run(&mut server, "data get storage t:t pos");
    assert_eq!(out, vec!["Storage t:t has the following contents: [1.3557453124999999d, -59.9d, 12.5d]".to_string()]);
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000005 Pos");
    assert_eq!(out, vec!["Minecart with Command Block has the following entity data: [1.6268516796874999d, -59.9d, 12.5d]".to_string()]);
}

#[test]
fn command_block_minecarts_tick_their_passengers_after_the_command() {
    let mut server = server();
    activator_line(&mut server, 14);
    run(&mut server, "summon pig 0.5 -60 14.5 {UUID:[I;1,2,3,7],NoAI:1b}");
    run(&mut server, "summon command_block_minecart 0.5 -60 14.5 {UUID:[I;1,2,3,6],Command:\"data merge entity 00000001-0000-0002-0000-000300000007 {Motion:[0.0d,5.0d,0.0d]}\"}");
    run(&mut server, "ride 00000001-0000-0002-0000-000300000007 mount 00000001-0000-0002-0000-000300000006");
    tick(&mut server, 4);
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000007 Motion");
    assert_eq!(out, vec!["Pig has the following entity data: [0.0d, 0.0d, 0.0d]".to_string()]);
}

#[test]
fn level_ticks_without_a_server_finish_suspended_command_block_minecart_ticks() {
    let mut server = server();
    activator_line(&mut server, 20);
    run(&mut server, "summon command_block_minecart 0.5 -60 20.5 {UUID:[I;1,2,3,5],Command:\"say hi\"}");
    for _ in 0..5 {
        server.levels[0].tick();
    }
    let level = &server.levels[0];
    assert!(!level.tick_suspended());
    assert!(!level.tick_continuation.is_suspended());
    let cart = entity_with_uuid(&server, 5).unwrap();
    assert_eq!(level.entity(cart).tick_count, 5);
}

#[test]
fn command_block_minecarts_riding_a_boat_resume_their_ride_tick_after_the_command() {
    let mut server = server();
    activator_line(&mut server, 18);
    run(&mut server, "summon oak_boat 0.5 -60 18.5 {UUID:[I;1,2,3,9]}");
    run(&mut server, "summon command_block_minecart 0.5 -60 18.5 {UUID:[I;1,2,3,5],Command:\"tp @e[type=oak_boat,limit=1,sort=nearest] ~5 ~ ~\"}");
    run(&mut server, "ride 00000001-0000-0002-0000-000300000005 mount 00000001-0000-0002-0000-000300000009");
    tick(&mut server, 4);
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000005 Pos");
    assert_eq!(out, vec!["Minecart with Command Block has the following entity data: [5.5d, -59.75d, 18.5d]".to_string()]);
}

#[test]
fn killed_command_block_minecarts_finish_their_tick_and_still_push_minecarts() {
    let mut server = server();
    activator_line(&mut server, 16);
    run(&mut server, "summon command_block_minecart 0.5 -60 16.5 {UUID:[I;1,2,3,5],Command:\"kill @s\"}");
    run(&mut server, "summon minecart 1.2 -60 16.5 {UUID:[I;1,2,3,8]}");
    tick(&mut server, 4);
    assert!(entity_with_uuid(&server, 5).is_none());
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000008 Motion");
    assert_eq!(out, vec!["Minecart has the following entity data: [0.05634302975382447d, 0.0d, 0.0d]".to_string()]);
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000008 Pos");
    assert_eq!(out, vec!["Minecart has the following entity data: [1.4323773385817005d, -59.9375d, 16.5d]".to_string()]);
}

