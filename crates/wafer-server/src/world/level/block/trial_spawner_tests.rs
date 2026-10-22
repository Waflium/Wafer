use wafer_core::{BlockPos, ChunkPos, Vec3};
use wafer_data::mob_effects;
use wafer_data::properties::{OMINOUS, TRIAL_SPAWNER_STATE, TrialSpawnerState};
use wafer_protocol::nbt::Nbt;

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::core::holder::RegistryElement;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::entity::LevelEntity;
use crate::world::level::BlockGetter;
use crate::world::level::block::entity::trialspawner::trial_spawner_config::TrialSpawnerConfig;

fn server() -> Server {
    let mut server = testing::server();
    for x in -1..=1 {
        for z in -1..=1 {
            server.levels[0].chunks.chunk(ChunkPos::new(x, z));
        }
    }
    run(&mut server, "forceload add 0 0 15 15");
    run(&mut server, "difficulty easy");
    run(&mut server, "gamerule spawn_mobs true");
    for _ in 0..2 {
        server.tick();
    }
    server
}

fn run(server: &mut Server, command: &str) {
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
}

fn state(server: &mut Server, pos: BlockPos) -> TrialSpawnerState {
    server.levels[0].block_state(pos).get(TRIAL_SPAWNER_STATE)
}

fn tick_until(server: &mut Server, pos: BlockPos, target: TrialSpawnerState, limit: i32) {
    let mut ticks = 0;
    while state(server, pos) != target {
        server.tick();
        ticks += 1;
        assert!(ticks < limit, "{:?} after {ticks} ticks", state(server, pos));
    }
}

fn pigs(server: &Server) -> usize {
    server.levels[0].entities.values().filter(|e| matches!(e, LevelEntity::Mob(_)) && e.entity().entity_type == wafer_data::registry_id(wafer_data::registry::ENTITY_TYPE, "minecraft:pig")).filter(|e| !e.entity().is_removed()).count()
}

#[test]
fn vanilla_trial_spawner_configs_decode() {
    for (name, _) in wafer_data::TRIAL_SPAWNER_CONFIGS {
        assert!(TrialSpawnerConfig::lookup(name).is_some(), "{name}");
    }
    let breeze = TrialSpawnerConfig::lookup("minecraft:trial_chamber/breeze/normal").expect("the breeze config");
    assert_eq!(breeze.ticks_between_spawn, 20);
    assert_eq!(breeze.calculate_target_total_mobs(2), 4);
}

#[test]
fn trial_spawners_spawn_reward_and_cool_down() {
    let mut server = server();
    let pos = BlockPos::new(0, -60, 0);
    let player = testing::server_player(&mut server, Vec3::new(0.5, -60.0, 6.5), 0.0, 0.0);
    server.levels[0].player_mut(player).game_mode.game_type = crate::server::level::game_mode::GameType::Survival;
    run(
        &mut server,
        r#"setblock 0 -60 0 trial_spawner{normal_config:{spawn_potentials:[{data:{entity:{id:"minecraft:pig"}},weight:1}],total_mobs:1.0f,simultaneous_mobs:1.0f,loot_tables_to_eject:[{data:"minecraft:spawners/trial_chamber/consumables",weight:1}]},target_cooldown_length:200}"#,
    );
    tick_until(&mut server, pos, TrialSpawnerState::Active, 40);
    let mut ticks = 0;
    while pigs(&server) == 0 {
        server.tick();
        ticks += 1;
        assert!(ticks < 100);
    }
    run(&mut server, "kill @e[type=pig]");
    tick_until(&mut server, pos, TrialSpawnerState::WaitingForRewardEjection, 5);
    tick_until(&mut server, pos, TrialSpawnerState::EjectingReward, 45);
    tick_until(&mut server, pos, TrialSpawnerState::Cooldown, 65);
    let data = server.levels[0].block_entity_data(pos).expect("the trial spawner");
    assert!(data.get("cooldown_ends_at").is_some());
    assert!(data.get("registered_players").is_none());
    assert_eq!(data.get("target_cooldown_length"), Some(&Nbt::Int(200)));
    tick_until(&mut server, pos, TrialSpawnerState::WaitingForPlayers, 200);
}

#[test]
fn bad_omen_turns_trial_spawners_ominous() {
    let mut server = server();
    let pos = BlockPos::new(0, -60, 0);
    let player = testing::server_player(&mut server, Vec3::new(0.5, -60.0, 6.5), 0.0, 0.0);
    server.levels[0].player_mut(player).game_mode.game_type = crate::server::level::game_mode::GameType::Survival;
    run(&mut server, r#"setblock 0 -60 0 trial_spawner{normal_config:"minecraft:trial_chamber/breeze/normal"}"#);
    run(&mut server, "effect give @a bad_omen 100 1");
    for _ in 0..40 {
        server.tick();
    }
    assert!(server.levels[0].block_state(pos).get(OMINOUS));
    assert!(!server.levels[0].has_effect(player, mob_effects::BAD_OMEN));
    let trial_omen = server.levels[0].living(player).effect(mob_effects::TRIAL_OMEN).expect("trial omen");
    assert!(trial_omen.duration > 35000);
    let data = server.levels[0].block_entity_data(pos).expect("the trial spawner");
    assert_eq!(data.get("normal_config"), Some(&Nbt::String("minecraft:trial_chamber/breeze/normal".into())));
}
