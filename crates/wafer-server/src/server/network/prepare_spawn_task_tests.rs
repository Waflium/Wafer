use wafer_core::{BlockPos, ChunkPos, Direction};
use wafer_data::blocks;
use wafer_data::packets::play::clientbound;
use wafer_protocol::Reader;
use wafer_protocol::nbt::Nbt;

use super::prepare_spawn_task::StartedBy;
use crate::ServerConfig;
use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::network::configuration::CommonListenerCookie;
use crate::network::connection::Outbound;
use crate::network::login::Profile;
use crate::server::Event;
use crate::server::level::testing;
use crate::world::level::block::entity::piston_moving_block_entity::PistonMovingBlockEntity;
use crate::world::level::block::moving_piston_block;
use crate::world::level::storage::{format_uuid, write_compressed};

const UUID: u128 = 0x1234;

fn world_with_broken_player(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("wafer-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("players/data")).unwrap();
    let frog = Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:frog".into())), ("Variant".into(), Nbt::Int(99))].into());
    let player = Nbt::Compound(vec![("DataVersion".into(), Nbt::Int(3086)), ("ShoulderEntityLeft".into(), frog)].into());
    write_compressed(&player, &dir.join(format!("players/data/{}.dat", format_uuid(UUID))), &[]).unwrap();
    dir
}

#[test]
fn a_data_fixer_exception_fails_the_configuration_task() {
    let dir = world_with_broken_player("prepare-spawn-fixer");
    let mut server = testing::server_with(ServerConfig { world: Some(dir.clone()), ..testing::flat_config() });
    let (reply, mut started) = tokio::sync::oneshot::channel();
    server.start_prepare_spawn(UUID, "Steve", StartedBy::SelectKnownPacks, reply);
    assert_eq!(started.try_recv(), Ok(Err(super::prepare_spawn_task::Disconnection::Handled)));
    assert!(server.prepare_spawn_tasks.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn a_data_fixer_exception_while_placing_disconnects_the_player() {
    let dir = world_with_broken_player("place-player-fixer");
    let mut server = testing::server_with(ServerConfig { world: Some(dir.clone()), ..testing::flat_config() });
    let (out, mut packets) = Outbound::capture();
    let profile = Profile { uuid: UUID, name: "Steve".into(), properties: Vec::new() };
    server.handle(Event::Join { conn: 1, profile, cookie: CommonListenerCookie::create_initial(false), out, address: "127.0.0.1:25565".parse().unwrap() });
    let ids: Vec<i32> = std::iter::from_fn(|| packets.try_recv().ok()).map(|p| Reader::new(&p).varint().unwrap()).collect();
    assert_eq!(ids, vec![clientbound::DISCONNECT]);
    assert!(server.player_list.is_empty());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn the_join_spawn_search_finishes_through_server_tasks() {
    use crate::server::level::player_spawn_finder::Finished;
    let mut server = testing::server_with(testing::flat_config());
    let id = server.find_spawn(0, wafer_core::BlockPos::new(0, -60, 0));
    assert!(server.spawn_finders[&id].finished().is_none());
    server.managed_block(|server| server.spawn_finders[&id].finished().is_some());
    assert!(matches!(server.spawn_finders[&id].finished(), Some(Finished::Spawn(_))));
}

#[test]
fn a_closed_join_spawn_search_is_dropped_once_its_last_candidate_ran() {
    let mut server = testing::server_with(testing::flat_config());
    let (reply, _started) = tokio::sync::oneshot::channel();
    server.start_prepare_spawn(UUID, "Steve", StartedBy::SelectKnownPacks, reply);
    assert_eq!(server.spawn_finders.len(), 1);
    server.close_prepare_spawn(UUID);
    server.managed_block(|server| server.spawn_finders.is_empty());
    assert!(server.prepare_spawn_tasks.is_empty());
}

#[test]
fn a_cancelled_spawn_search_never_finishes_with_a_position() {
    use crate::server::level::player_spawn_finder::Finished;
    let mut server = testing::server_with(testing::flat_config());
    let id = server.find_spawn(0, wafer_core::BlockPos::new(0, -60, 0));
    server.spawn_finders.get_mut(&id).unwrap().cancel();
    server.managed_block(|server| server.spawn_finders[&id].finished().is_some());
    assert!(matches!(server.spawn_finders[&id].finished(), Some(Finished::Cancelled)));
}

#[test]
fn spawn_candidates_inside_a_boat_are_rejected() {
    use crate::server::level::player_spawn_finder::Finished;
    let mut server = testing::server();
    server.levels[0].chunks.chunk(ChunkPos::new(0, 0));
    run(&mut server, "forceload add 0 0");
    server.levels[0].finish_chunk_work();
    server.tick();
    run(&mut server, "gamerule respawn_radius 0");
    run(&mut server, "summon oak_boat 8.5 -60 8.5");
    let id = server.find_spawn(0, BlockPos::new(8, -60, 8));
    server.managed_block(|server| server.spawn_finders[&id].finished().is_some());
    let Some(Finished::Spawn(position)) = server.spawn_finders[&id].finished() else { panic!("the spawn search should succeed") };
    assert_eq!((position.x, position.y, position.z), (8.5, -59.0, 8.5));
}

fn run(server: &mut crate::server::Server, command: &str) {
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
}

fn moving_piston_moving_a_moving_piston(server: &mut crate::server::Server) -> BlockPos {
    server.levels[0].chunks.chunk(ChunkPos::new(0, 0));
    run(server, "forceload add 0 0");
    server.levels[0].finish_chunk_work();
    server.tick();
    run(server, "setblock 8 -50 8 piston[facing=up]");
    run(server, "setblock 9 -50 8 redstone_block");
    server.tick();
    let pos = BlockPos::new(8, -49, 8);
    *server.levels[0].piston_moving_block_entity(pos).expect("a moving piston") = PistonMovingBlockEntity::new(blocks::MOVING_PISTON.default_state(), Direction::Up, true, false);
    pos
}

#[test]
fn a_join_spawn_search_failing_inside_start_ticks_the_task_once_more_like_vanilla() {
    use super::prepare_spawn_task::Disconnection;
    for started_by in [StartedBy::SelectKnownPacks, StartedBy::AcceptCodeOfConduct, StartedBy::ResourcePackResponse] {
        let mut server = testing::server();
        let pos = moving_piston_moving_a_moving_piston(&mut server);
        run(&mut server, "gamerule respawn_radius 0");
        server.spawn.pos = pos;
        let (reply, mut started) = tokio::sync::oneshot::channel();
        server.start_prepare_spawn(UUID, "Steve", started_by, reply);
        server.managed_block(|server| server.spawn_finders.values().all(|finder| finder.finished().is_some()));
        server.tick_prepare_spawns(false);
        assert!(started.try_recv().is_err());
        server.tick_prepare_spawns(false);
        assert_eq!(started.try_recv(), Ok(Err(Disconnection::HandledTwice)));
        assert!(server.prepare_spawn_tasks.is_empty());
        assert!(server.spawn_finders.is_empty());
    }
}

#[test]
fn a_moving_piston_moving_a_moving_piston_fails_the_spawn_search_with_a_reported_stack_overflow() {
    use crate::server::level::player_spawn_finder::Finished;
    let mut server = testing::server();
    let pos = moving_piston_moving_a_moving_piston(&mut server);
    let overflow = crate::util::catch_throwable(|| moving_piston_block::get_collision_shape(&server.levels[0], pos)).unwrap_err();
    assert_eq!(overflow.description, "java.lang.StackOverflowError");
    assert_eq!(overflow.stack_trace.len(), 1024);
    assert_eq!(overflow.stack_trace[0], "net.minecraft.server.level.ServerChunkCache.getChunk(ServerChunkCache.java:132)");
    assert_eq!(overflow.stack_trace[1023], "net.minecraft.world.level.block.piston.MovingPistonBlock.getCollisionShape(MovingPistonBlock.java:103)");
    run(&mut server, "gamerule respawn_radius 0");
    let id = server.find_spawn(0, pos);
    server.managed_block(|server| server.spawn_finders[&id].finished().is_some());
    let Some(Finished::Failed(reported)) = server.spawn_finders[&id].finished() else { panic!("the spawn search should fail") };
    let trace = crate::server::level::player_spawn_finder::completion_exception(reported, Vec::new()).print_stack_trace();
    assert!(trace.starts_with(
        "java.util.concurrent.CompletionException: net.minecraft.ReportedException: Searching for spawn\n\
        \tat java.base/java.util.concurrent.CompletableFuture.wrapInCompletionException(CompletableFuture.java:323)\n"
    ));
    assert!(trace.contains(
        "\nCaused by: net.minecraft.ReportedException: Searching for spawn\n\
        \tat net.minecraft.server.level.PlayerSpawnFinder.lambda$scheduleCandidate$0(PlayerSpawnFinder.java:149)\n\
        \tat java.base/java.util.concurrent.CompletableFuture.uniWhenComplete(CompletableFuture.java:884)\n"
    ));
    assert!(trace.contains("\nCaused by: java.lang.StackOverflowError\n\tat net.minecraft.server.level.ServerChunkCache.getChunk(ServerChunkCache.java:132)\n"));
}
