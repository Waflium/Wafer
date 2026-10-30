use tokio::sync::mpsc::UnboundedReceiver;
use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::blocks;
use wafer_data::packets::play::clientbound;
use wafer_data::properties::{BED_PART, BedPart, HORIZONTAL_FACING};
use wafer_protocol::Reader;

use crate::network::configuration::ClientInfo;
use crate::network::connection::Outbound;
use crate::network::login::Profile;
use crate::server::level::ServerLevel;
use crate::server::level::events::{PARTICLES_AND_SOUND_DESTROY_BLOCK, PARTICLES_DESTROY_BLOCK};
use crate::server::level::server_player::ServerPlayer;
use crate::server::level::testing;
use crate::world::level::update_flags::UPDATE_ALL;

fn level_with_observer() -> (ServerLevel, UnboundedReceiver<Vec<u8>>) {
    let mut level = testing::level();
    let (out, mut packets) = Outbound::capture();
    let mut player = ServerPlayer::new(Profile { uuid: 1, name: "Tester".into(), properties: Vec::new() }, ClientInfo::default(), out, BlockPos::ZERO);
    player.entity.set_pos(Vec3::new(8.5, -60.0, 8.5));
    level.add_player(player);
    level.finish_chunk_work();
    while packets.try_recv().is_ok() {}
    (level, packets)
}

fn level_events(packets: &mut UnboundedReceiver<Vec<u8>>) -> Vec<(i32, BlockPos)> {
    let mut events = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        let mut r = Reader::new(&packet);
        if r.varint().unwrap() == clientbound::LEVEL_EVENT {
            let event = r.i32().unwrap();
            let (x, y, z) = r.position().unwrap();
            events.push((event, BlockPos::new(x, y, z)));
        }
    }
    events
}

fn place_bed(level: &mut ServerLevel, block: wafer_data::Block, foot: BlockPos) -> BlockPos {
    let head = foot.relative(Direction::South);
    let state = block.default_state().set(HORIZONTAL_FACING, Direction::South);
    level.set_block(foot, state.set(BED_PART, BedPart::Foot), UPDATE_ALL);
    level.set_block(head, state.set(BED_PART, BedPart::Head), UPDATE_ALL);
    head
}

#[test]
fn destroying_a_straw_bed_sends_the_destroy_block_event() {
    let (mut level, mut packets) = level_with_observer();
    let foot = BlockPos::new(9, -60, 8);
    let head = place_bed(&mut level, blocks::STRAW_BED, foot);
    while packets.try_recv().is_ok() {}
    assert!(level.destroy_block(foot, false, None, 512));
    assert_eq!(level_events(&mut packets), vec![(PARTICLES_DESTROY_BLOCK, foot), (PARTICLES_DESTROY_BLOCK, head)]);
}

#[test]
fn destroying_a_bed_sends_the_destroy_block_and_sound_event() {
    let (mut level, mut packets) = level_with_observer();
    let foot = BlockPos::new(9, -60, 8);
    let head = place_bed(&mut level, blocks::RED_BED, foot);
    while packets.try_recv().is_ok() {}
    assert!(level.destroy_block(foot, false, None, 512));
    assert_eq!(level_events(&mut packets), vec![(PARTICLES_AND_SOUND_DESTROY_BLOCK, foot), (PARTICLES_AND_SOUND_DESTROY_BLOCK, head)]);
}
