use wafer_data::packets::play::clientbound;
use wafer_protocol::Reader;
use wafer_protocol::nbt::Nbt;

use crate::ServerConfig;
use crate::network::configuration::CommonListenerCookie;
use crate::network::connection::Outbound;
use crate::network::login::Profile;
use crate::server::Event;
use crate::server::level::testing;
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
    server.start_prepare_spawn(UUID, "Steve", reply);
    assert_eq!(started.try_recv(), Ok(Err(())));
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
fn a_cancelled_spawn_search_never_finishes_with_a_position() {
    use crate::server::level::player_spawn_finder::Finished;
    let mut server = testing::server_with(testing::flat_config());
    let id = server.find_spawn(0, wafer_core::BlockPos::new(0, -60, 0));
    server.spawn_finders.get_mut(&id).unwrap().cancel();
    while server.poll_task() {}
    assert!(matches!(server.spawn_finders[&id].finished(), Some(Finished::Failed)));
}
