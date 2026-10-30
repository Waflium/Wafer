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
    server.levels[0].finish_chunk_work();
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

#[test]
fn sculk_catalysts_take_experience_from_dying_mobs() {
    let mut server = server();
    run(&mut server, "difficulty easy");
    run(&mut server, "setblock 0 -61 0 sculk_catalyst");
    run(&mut server, "summon zombie 3 -60 0 {NoAI:1b}");
    run(&mut server, "kill @e[type=zombie]");
    let level = &mut server.levels[0];
    assert!(level.block_state(BlockPos::new(0, -61, 0)).get(wafer_data::properties::BLOOM));
    let mut output = Vec::new();
    if let Some(entity) = level.chunks.block_entity_ref(BlockPos::new(0, -61, 0))
        && let crate::world::level::block::entity::BlockEntityKind::SculkCatalyst(catalyst) = &entity.kind
    {
        catalyst.save_additional(&mut output);
    }
    let cursors = output.iter().find(|(name, _)| name == "cursors").map(|(_, nbt)| format!("{nbt:?}")).unwrap_or_default();
    assert!(cursors.contains("Int(5)"), "{cursors}");
    for _ in 0..30 {
        server.tick();
    }
    assert!(!server.levels[0].entities.values().any(|entity| matches!(entity, crate::world::entity::LevelEntity::ExperienceOrb(_))));
}

#[test]
fn sculk_shriekers_warn_players_and_answer_with_darkness() {
    let mut server = server();
    run(&mut server, "difficulty easy");
    run(&mut server, "setblock 2 -60 0 sculk_shrieker[can_summon=true]");
    let player = testing::player(&mut server.levels[0], wafer_core::Vec3::new(2.5, -60.0, 4.5), 0.0, 0.0);
    let shrieker = BlockPos::new(2, -60, 0);
    server.levels[0].sculk_shrieker_try_shriek(shrieker, Some(player));
    assert!(server.levels[0].block_state(shrieker).get(wafer_data::properties::SHRIEKING));
    assert_eq!(server.levels[0].player(player).warden_spawn_tracker.get_warning_level(), 1);
    for _ in 0..91 {
        server.tick();
    }
    assert!(!server.levels[0].block_state(shrieker).get(wafer_data::properties::SHRIEKING));
    assert!(server.levels[0].has_effect(player, wafer_data::mob_effects::DARKNESS));
}
