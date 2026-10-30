use tokio::sync::mpsc::UnboundedReceiver;
use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;
use wafer_data::packets::play::clientbound;
use wafer_protocol::Reader;

use crate::network::configuration::ClientInfo;
use crate::network::connection::Outbound;
use crate::network::login::Profile;
use crate::server::level::ServerLevel;
use crate::server::level::events::{PARTICLES_AND_SOUND_DESTROY_BLOCK, SOUND_EXTINGUISH_FIRE};
use crate::server::level::server_player::ServerPlayer;
use crate::server::level::testing;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::block;
use crate::world::level::update_flags::UPDATE_ALL;

fn level_with_player() -> (ServerLevel, EntityId, UnboundedReceiver<Vec<u8>>) {
    let mut level = testing::level();
    let (out, mut packets) = Outbound::capture();
    let mut player = ServerPlayer::new(Profile { uuid: 1, name: "Tester".into(), properties: Vec::new() }, ClientInfo::default(), out, BlockPos::ZERO);
    player.entity.set_pos(Vec3::new(8.5, -60.0, 8.5));
    let id = player.entity.id;
    level.add_player(player);
    level.finish_chunk_work();
    while packets.try_recv().is_ok() {}
    (level, id, packets)
}

fn level_events(packets: &mut UnboundedReceiver<Vec<u8>>) -> Vec<(i32, BlockPos, i32)> {
    let mut events = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        let mut r = Reader::new(&packet);
        if r.varint().unwrap() == clientbound::LEVEL_EVENT {
            let event = r.i32().unwrap();
            let (x, y, z) = r.position().unwrap();
            events.push((event, BlockPos::new(x, y, z), r.i32().unwrap()));
        }
    }
    events
}

#[test]
fn destroying_fire_spawns_no_destroy_particles() {
    let (mut level, _player, mut packets) = level_with_player();
    let pos = BlockPos::new(9, -60, 8);
    level.set_block(pos, blocks::FIRE.default_state(), UPDATE_ALL);
    while packets.try_recv().is_ok() {}
    assert!(level.destroy_block(pos, false, None, 512));
    assert!(level_events(&mut packets).iter().all(|&(event, ..)| event != PARTICLES_AND_SOUND_DESTROY_BLOCK));
    let pos = BlockPos::new(10, -60, 8);
    level.set_block(pos.below(), blocks::SOUL_SOIL.default_state(), UPDATE_ALL);
    level.set_block(pos, blocks::SOUL_FIRE.default_state(), UPDATE_ALL);
    while packets.try_recv().is_ok() {}
    assert!(level.destroy_block(pos, false, None, 512));
    assert!(level_events(&mut packets).iter().all(|&(event, ..)| event != PARTICLES_AND_SOUND_DESTROY_BLOCK));
}

#[test]
fn destroying_other_blocks_still_spawns_destroy_particles() {
    let (mut level, _player, mut packets) = level_with_player();
    let pos = BlockPos::new(9, -60, 8);
    level.set_block(pos, blocks::STONE.default_state(), UPDATE_ALL);
    while packets.try_recv().is_ok() {}
    assert!(level.destroy_block(pos, false, None, 512));
    assert_eq!(level_events(&mut packets), vec![(PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, blocks::STONE.default_state().id() as i32)]);
}

#[test]
fn breaking_fire_plays_the_extinguish_sound_only() {
    let (mut level, player, mut packets) = level_with_player();
    let pos = BlockPos::new(9, -60, 8);
    let fire = blocks::FIRE.default_state();
    level.set_block(pos, fire, UPDATE_ALL);
    while packets.try_recv().is_ok() {}
    block::of(fire).player_will_destroy(&mut level, pos, fire, player);
    assert_eq!(level_events(&mut packets), vec![(SOUND_EXTINGUISH_FIRE, pos, 0)]);
}
