use wafer_core::{BlockPos, ChunkPos};
use wafer_data::properties::{POWER, SCULK_SENSOR_PHASE, SculkSensorPhase};

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
    for _ in 0..2 {
        server.tick();
    }
    server
}

fn run(server: &mut Server, command: &str) {
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
}

fn phase(server: &mut Server, pos: BlockPos) -> SculkSensorPhase {
    server.levels[0].block_state(pos).get(SCULK_SENSOR_PHASE)
}

#[test]
fn sculk_sensors_hear_note_blocks_and_cool_down() {
    let mut server = server();
    let sensor = BlockPos::new(0, -60, 0);
    run(&mut server, "setblock 0 -60 0 sculk_sensor");
    run(&mut server, "setblock 4 -60 0 note_block");
    for _ in 0..2 {
        server.tick();
    }
    run(&mut server, "setblock 5 -60 0 redstone_block");
    let mut ticks = 0;
    while phase(&mut server, sensor) != SculkSensorPhase::Active {
        server.tick();
        ticks += 1;
        assert!(ticks < 20);
    }
    assert!(server.levels[0].block_state(sensor).get(POWER) > 0);
    assert_eq!(server.levels[0].sculk_sensor_last_vibration_frequency(sensor), Some(10));
    for _ in 0..30 {
        server.tick();
    }
    assert_eq!(phase(&mut server, sensor), SculkSensorPhase::Cooldown);
    for _ in 0..10 {
        server.tick();
    }
    assert_eq!(phase(&mut server, sensor), SculkSensorPhase::Inactive);
}
