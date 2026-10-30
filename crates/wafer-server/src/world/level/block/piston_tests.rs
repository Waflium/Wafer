use wafer_core::{Axis, BlockPos, ChunkPos, Vec3};
use wafer_data::blocks;
use wafer_data::properties::EXTENDED;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::level::BlockGetter;
use crate::world::level::clip::{ClipBlock, ClipContext, ClipFluid};
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::phys::shapes::collision_context::CollisionContext;

fn server() -> Server {
    let mut server = testing::server();
    for x in -1..=2 {
        for z in -1..=2 {
            server.levels[0].chunks.chunk(ChunkPos::new(x, z));
        }
    }
    run(&mut server, "forceload add 0 0 31 31");
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

fn state(server: &mut Server, x: i32, y: i32, z: i32) -> wafer_data::BlockState {
    server.levels[0].block_state(BlockPos::new(x, y, z))
}

#[test]
fn pistons_push_blocks_and_sticky_pistons_pull_them_back() {
    let mut server = server();
    run(&mut server, "setblock 0 -50 0 piston[facing=east]");
    run(&mut server, "setblock 1 -50 0 stone");
    run(&mut server, "setblock 0 -50 4 sticky_piston[facing=east]");
    run(&mut server, "setblock 1 -50 4 slime_block");
    run(&mut server, "setblock 1 -49 4 oak_planks");
    run(&mut server, "setblock 0 -50 -1 redstone_block");
    run(&mut server, "setblock 0 -50 3 redstone_block");
    server.tick();
    assert!(state(&mut server, 2, -50, 0).is_block(blocks::MOVING_PISTON));
    for _ in 0..4 {
        server.tick();
    }
    assert!(state(&mut server, 0, -50, 0).get(EXTENDED));
    assert!(state(&mut server, 1, -50, 0).is_block(blocks::PISTON_HEAD));
    assert!(state(&mut server, 2, -50, 0).is_block(blocks::STONE));
    assert!(state(&mut server, 2, -50, 4).is_block(blocks::SLIME_BLOCK));
    assert!(state(&mut server, 2, -49, 4).is_block(blocks::OAK_PLANKS));
    run(&mut server, "setblock 0 -50 -1 air");
    run(&mut server, "setblock 0 -50 3 air");
    for _ in 0..6 {
        server.tick();
    }
    assert!(!state(&mut server, 0, -50, 0).get(EXTENDED));
    assert!(state(&mut server, 1, -50, 0).is_air());
    assert!(state(&mut server, 2, -50, 0).is_block(blocks::STONE));
    assert!(state(&mut server, 1, -50, 4).is_block(blocks::SLIME_BLOCK));
    assert!(state(&mut server, 1, -49, 4).is_block(blocks::OAK_PLANKS));
    assert!(state(&mut server, 2, -50, 4).is_air());
}

#[test]
fn pistons_stop_at_thirteen_blocks_and_immovable_blocks() {
    let mut server = server();
    run(&mut server, "fill 1 -50 8 13 -50 8 stone");
    run(&mut server, "setblock 0 -50 8 piston[facing=east]");
    run(&mut server, "setblock 0 -50 12 piston[facing=east]");
    run(&mut server, "setblock 1 -50 12 obsidian");
    run(&mut server, "setblock 0 -50 7 redstone_block");
    run(&mut server, "setblock 0 -50 11 redstone_block");
    for _ in 0..4 {
        server.tick();
    }
    assert!(!state(&mut server, 0, -50, 8).get(EXTENDED));
    assert!(!state(&mut server, 0, -50, 12).get(EXTENDED));
}

#[test]
fn pistons_lift_entities_half_a_block_per_tick() {
    let mut server = server();
    run(&mut server, "setblock 6 -50 6 piston[facing=up]");
    run(&mut server, "summon pig 6.5 -49 6.5 {NoAI:1b,UUID:[I;1,2,3,1]}");
    run(&mut server, "setblock 7 -50 6 redstone_block");
    server.tick();
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000001 Pos");
    assert_eq!(out, vec!["Pig has the following entity data: [6.5d, -48.49d, 6.5d]".to_string()]);
    server.tick();
    let out = run(&mut server, "data get entity 00000001-0000-0002-0000-000300000001 Pos");
    assert_eq!(out, vec!["Pig has the following entity data: [6.5d, -47.99d, 6.5d]".to_string()]);
}

#[test]
fn clips_and_pathfinding_see_the_live_shape_of_moving_pistons() {
    let mut server = server();
    run(&mut server, "setblock 12 -50 12 piston[facing=up]");
    run(&mut server, "setblock 13 -50 12 redstone_block");
    server.tick();
    let pos = BlockPos::new(12, -49, 12);
    let state = state(&mut server, 12, -49, 12);
    assert!(state.is_block(blocks::MOVING_PISTON));
    let level = &mut server.levels[0];
    let live = crate::world::level::block::moving_piston_block::get_collision_shape(level, pos);
    assert!(!live.is_empty());
    let top = pos.y as f64 + live.max(Axis::Y);
    assert_eq!(level.get_state_collision_shape(state, pos).max(Axis::Y), live.max(Axis::Y));
    assert_eq!(PathNavigationRegion::new(level).get_state_collision_shape(state, pos).max(Axis::Y), live.max(Axis::Y));
    assert_eq!(level.get_state_collision_shape_in_context(state, pos, &CollisionContext::EMPTY).max(Axis::Y), live.max(Axis::Y));
    for block in [ClipBlock::Collider, ClipBlock::Visual] {
        let context = ClipContext { from: Vec3::new(12.5, -45.0, 12.5), to: Vec3::new(12.5, -49.5, 12.5), block, fluid: ClipFluid::None, collision_context: CollisionContext::EMPTY };
        let result = level.clip(&context);
        assert!(!result.miss);
        assert_eq!(result.hit.block_pos, pos);
        assert!((result.hit.location.y - top).abs() < 1.0e-9, "{result:?} {top}");
    }
}


#[test]
fn carpet_duplicators_keep_running() {
    let mut server = server();
    for command in [
        "setblock 3 -58 5 oak_planks",
        "setblock 3 -58 6 oak_planks",
        "setblock 5 -58 5 slime_block",
        "setblock 5 -58 6 slime_block",
        "setblock 5 -57 6 slime_block",
        "setblock 5 -56 6 slime_block",
        "setblock 4 -58 5 observer[facing=east]",
        "setblock 4 -58 6 sticky_piston[facing=east]",
        "setblock 6 -58 6 dead_brain_coral_wall_fan[facing=east]",
        "setblock 3 -57 5 redstone_wire[north=side,south=side]",
        "setblock 3 -57 6 redstone_wire[north=side,south=side]",
        "setblock 2 -58 6 lever[face=wall,facing=west]",
        "setblock 6 -57 6 white_carpet",
        "setblock 6 -56 6 white_carpet",
    ] {
        run(&mut server, command);
    }
    for _ in 0..100 {
        server.tick();
    }
    let carpets: i32 = server.levels[0]
        .entities
        .values()
        .filter_map(|entity| match entity {
            crate::world::entity::LevelEntity::Item(item) if item.item.is(wafer_data::items::WHITE_CARPET) => Some(item.item.count),
            _ => None,
        })
        .sum();
    assert!(carpets > 10, "only {carpets} carpets were duplicated");
}
