use wafer_core::{BlockPos, ChunkPos};
use wafer_data::properties::{POTENT_SULFUR_STATE, PotentSulfurState};

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::level::BlockGetter;
use wafer_protocol::nbt::Nbt;

fn run(server: &mut Server, command: &str) {
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
}

#[test]
fn geysers_wait_erupt_and_go_dormant_again() {
    let mut server = testing::server();
    server.levels[0].chunks.chunk(ChunkPos::new(0, 0));
    run(&mut server, "forceload add 0 0");
    let pos = BlockPos::new(4, -61, 0);
    run(&mut server, "fill 3 -62 -1 5 -58 1 glass");
    run(&mut server, "setblock 4 -62 0 magma_block");
    run(&mut server, "setblock 4 -61 0 potent_sulfur");
    run(&mut server, "fill 4 -60 0 4 -58 0 water");
    assert_eq!(server.levels[0].block_state(pos).get(POTENT_SULFUR_STATE), PotentSulfurState::Dormant);
    run(&mut server, "data merge block 4 -61 0 {countdown:2}");
    let mut ticks = 0;
    while server.levels[0].block_state(pos).get(POTENT_SULFUR_STATE) != PotentSulfurState::Erupting {
        server.tick();
        ticks += 1;
        assert!(ticks < 60);
    }
    while server.levels[0].block_state(pos).get(POTENT_SULFUR_STATE) != PotentSulfurState::Dormant {
        server.tick();
        ticks += 1;
        assert!(ticks < 200);
    }
    let countdown = |server: &mut Server| server.levels[0].block_entity_data(pos).and_then(|data| data.get("countdown").cloned());
    assert_eq!(countdown(&mut server), Some(Nbt::Int(0)));
    for _ in 0..20 {
        server.tick();
    }
    assert!(matches!(countdown(&mut server), Some(Nbt::Int(value)) if value > 20));
}
