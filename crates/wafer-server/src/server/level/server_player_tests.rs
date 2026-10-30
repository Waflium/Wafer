use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;
use wafer_data::packets::play::clientbound;
use wafer_protocol::Reader;
use wafer_protocol::nbt::Nbt;

use super::server_player::ServerPlayer;
use super::testing;
use super::ticket::TicketType;
use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::network::configuration::ClientInfo;
use crate::network::connection::Outbound;
use crate::network::login::{Profile, offline_uuid};
use crate::server::Server;
use crate::world::entity::EntityId;
use crate::world::entity::monster::warden::warden_spawn_tracker::WardenSpawnTracker;
use crate::world::level::storage::uuid_ints;

const ORIGIN: Vec3 = Vec3::new(0.5, -60.0, 0.5);
const NETHER: usize = 2;

fn join(server: &mut Server, name: &str) -> EntityId {
    let profile = Profile { uuid: offline_uuid(name), name: name.into(), properties: Vec::new() };
    let id = testing::server_player_with_profile(server, profile, ORIGIN, 0.0, 0.0);
    server.name_to_id_cache.add(crate::server::players::NameAndId { id: offline_uuid(name), name: name.into() });
    server.levels[0].chunks.run_until_idle();
    server.levels[0].process_chunk_events();
    id
}

fn run(server: &mut Server, command: &str) {
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
}

fn doubles(values: [f64; 3]) -> Nbt {
    Nbt::List(values.into_iter().map(Nbt::Double).collect())
}

fn summon_pearl(server: &mut Server, level: usize, owner: u128, pos: [f64; 3], motion: [f64; 3]) -> EntityId {
    let tag = Nbt::Compound(vec![
        ("id".into(), Nbt::String("minecraft:ender_pearl".into())),
        ("Pos".into(), doubles(pos)),
        ("Motion".into(), doubles(motion)),
        ("Owner".into(), uuid_ints(owner)),
    ].into());
    let level = &mut server.levels[level];
    let pearl = level.load_entity(&tag).expect("an ender pearl");
    let id = pearl.entity().id;
    assert!(level.add_entity(pearl));
    id
}

fn has_ender_pearl_ticket(server: &Server, level: usize, pos: Vec3) -> bool {
    let chunk = BlockPos::containing_vec(pos).chunk().pack();
    server.levels[level].chunks.distance_manager.ticket_storage.tickets(chunk).iter().any(|t| t.ty == TicketType::EnderPearl)
}

#[test]
fn ender_pearls_are_saved_with_the_player_and_spawned_again() {
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    let pearl = summon_pearl(&mut server, 0, offline_uuid("Alice"), [0.5, -58.0, 0.5], [0.2, 1.5, 0.1]);
    let pearl_uuid = server.levels[0].entity(pearl).uuid;
    assert_eq!(server.player(alice).ender_pearls, [pearl]);

    let saved = server.save_player_with_ender_pearls(alice);
    let Nbt::Compound(fields) = &saved else { unreachable!() };
    let keys: Vec<&str> = fields.iter().map(|(k, _)| k.as_str()).collect();
    let at = |key: &str| keys.iter().position(|&k| k == key).unwrap();
    assert!(at("spawn_extra_particles_on_fall") < at("ender_pearls") && at("ender_pearls") < at("post_effects"));
    let pearls = saved.get("ender_pearls").unwrap().as_list();
    assert_eq!(pearls.len(), 1);
    assert_eq!(pearls[0].get("id"), Some(&Nbt::String("minecraft:ender_pearl".into())));
    assert_eq!(pearls[0].get("ender_pearl_dimension"), Some(&Nbt::String("minecraft:overworld".into())));
    assert_eq!(pearls[0].get("Owner"), Some(&uuid_ints(offline_uuid("Alice"))));
    assert_eq!(pearls[0].get("UUID"), Some(&uuid_ints(pearl_uuid)));
    assert_eq!(pearls[0].get("Motion"), Some(&doubles([0.2, 1.5, 0.1])));

    server.remove_ender_pearls(alice);
    assert!(server.levels[0].entity_ref(pearl).is_none());
    assert_eq!(server.player(alice).ender_pearls, [pearl]);

    server.player_mut(alice).ender_pearls.clear();
    server.load_and_spawn_ender_pearls(&saved);
    let respawned = server.player(alice).ender_pearls.clone();
    assert_eq!(respawned.len(), 1);
    let e = server.levels[0].entity(respawned[0]);
    assert_eq!((e.uuid, e.position()), (pearl_uuid, Vec3::new(0.5, -58.0, 0.5)));
    assert!(has_ender_pearl_ticket(&server, 0, Vec3::new(0.5, -58.0, 0.5)));
}

#[test]
fn ender_pearls_without_a_loaded_level_are_skipped() {
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    let saved = Nbt::Compound(vec![(
        "ender_pearls".into(),
        Nbt::List(vec![
            Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:ender_pearl".into())), ("ender_pearl_dimension".into(), Nbt::String("minecraft:the_moon".into()))].into()),
            Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:ender_pearl".into()))].into()),
        ]),
    )].into());
    let before = server.levels.iter().map(|l| l.entities.len()).sum::<usize>();
    server.load_and_spawn_ender_pearls(&saved);
    assert_eq!(server.levels.iter().map(|l| l.entities.len()).sum::<usize>(), before);
    assert!(server.player(alice).ender_pearls.is_empty());
}

#[test]
fn removed_ender_pearls_are_not_saved() {
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    let pearl = summon_pearl(&mut server, 0, offline_uuid("Alice"), [0.5, -58.0, 0.5], [0.0, 0.0, 0.0]);
    server.levels[0].set_removed(pearl, crate::world::entity::RemovalReason::UnloadedWithPlayer);
    let saved = server.save_player_with_ender_pearls(alice);
    assert_eq!(saved.get("ender_pearls"), Some(&Nbt::List(Vec::new())));
}

#[test]
fn a_pearl_lands_and_brings_its_owner_back_from_another_dimension() {
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    let pearl = summon_pearl(&mut server, 0, offline_uuid("Alice"), [0.5, -58.0, 0.5], [0.0, 0.0, 0.0]);
    server.levels[0].entity_mut(pearl).data.set(crate::world::entity::data::ids::NO_GRAVITY, crate::world::entity::data::DataValue::Boolean(true));
    server.tick();
    run(&mut server, "execute in minecraft:the_nether run tp Alice 0.5 128 0.5");
    assert_eq!(server.level_of(alice), Some(NETHER));
    server.player_mut(alice).ender_pearls.clear();
    server.levels[0].entity_mut(pearl).data.set(crate::world::entity::data::ids::NO_GRAVITY, crate::world::entity::data::DataValue::Boolean(false));
    server.levels[0].entity_mut(pearl).set_pos(Vec3::new(0.5, -58.0, 0.5));
    server.levels[0].entity_mut(pearl).set_delta_movement(Vec3::new(0.2, 1.5, 0.1));
    for _ in 0..40 {
        server.tick();
    }
    assert_eq!(server.player(alice).ender_pearls, [pearl]);
    let mut ticks = 40;
    while server.level_of(alice) == Some(NETHER) {
        assert!(ticks < 200, "the pearl never landed");
        server.tick();
        ticks += 1;
    }
    assert_eq!(server.level_of(alice), Some(0));
    assert_eq!(server.levels[0].entity(alice).position(), Vec3::new(12.123600777423414, -59.57260811774503, 6.311800388711707));
    assert!(server.levels[0].player(alice).health < 20.0);
    server.tick();
    assert!(server.player(alice).ender_pearls.is_empty());
}

#[test]
fn a_pearl_keeps_its_chunk_loaded_while_the_owner_is_elsewhere() {
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    let pearl = summon_pearl(&mut server, 0, offline_uuid("Alice"), [0.5, 0.0, 0.5], [0.0, 0.0, 0.0]);
    server.levels[0].entity_mut(pearl).data.set(crate::world::entity::data::ids::NO_GRAVITY, crate::world::entity::data::DataValue::Boolean(true));
    server.tick();
    run(&mut server, "execute in minecraft:the_nether run tp Alice 0.5 128 0.5");
    let ticks = server.levels[0].entity(pearl).tick_count;
    for _ in 0..400 {
        server.tick();
    }
    assert_eq!(server.level_of(alice), Some(NETHER));
    assert_eq!(server.levels[0].entity(pearl).tick_count, ticks + 400);
    assert!(has_ender_pearl_ticket(&server, 0, Vec3::new(0.5, 0.0, 0.5)));
}

#[test]
fn logging_out_unloads_the_owners_pearls() {
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    let (out, _packets) = Outbound::capture();
    server.levels[0].player_mut(alice).connection = out;
    server.connections.insert(0, alice);
    run(&mut server, "execute in minecraft:the_nether run tp Alice 0.5 128 0.5");
    let pearl = summon_pearl(&mut server, 0, offline_uuid("Alice"), [0.5, -58.0, 0.5], [0.0, 0.0, 0.0]);
    let other = summon_pearl(&mut server, 0, 7, [0.5, -58.0, 0.5], [0.0, 0.0, 0.0]);
    server.remove_player(0, None);
    assert!(server.levels[0].entity_ref(pearl).is_none());
    assert!(server.levels[0].entity_ref(other).is_some());
}

#[test]
fn warden_spawn_tracker_ticks_and_survives_saving_and_respawn() {
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    server.player_mut(alice).warden_spawn_tracker = WardenSpawnTracker::new(11999, 3, 2);
    server.tick();
    assert_eq!(server.player(alice).warden_spawn_tracker, WardenSpawnTracker::new(12000, 3, 1));
    server.tick();
    assert_eq!(server.player(alice).warden_spawn_tracker, WardenSpawnTracker::new(0, 2, 0));

    server.player_mut(alice).spawn_extra_particles_on_fall = true;
    let saved = server.save_player_with_ender_pearls(alice);
    assert_eq!(saved.get("warden_spawn_tracker"), Some(&WardenSpawnTracker::new(0, 2, 0).to_nbt()));
    assert_eq!(saved.get("spawn_extra_particles_on_fall"), Some(&Nbt::Byte(1)));
    let mut loaded = ServerPlayer::new(Profile { uuid: 2, name: "Bob".into(), properties: Vec::new() }, ClientInfo::default(), Outbound::detached(), BlockPos::ZERO);
    loaded.load_nbt(&saved).unwrap();
    assert_eq!((loaded.warden_spawn_tracker, loaded.spawn_extra_particles_on_fall), (WardenSpawnTracker::new(0, 2, 0), true));

    server.respawn(alice, false, crate::world::entity::RemovalReason::Killed);
    assert_eq!(server.player(alice).warden_spawn_tracker, WardenSpawnTracker::new(0, 2, 0));
    assert!(!server.player(alice).spawn_extra_particles_on_fall);
}

#[test]
fn players_with_a_non_finite_position_fail_to_load() {
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    let Nbt::Compound(mut fields) = server.save_player_with_ender_pearls(alice) else { unreachable!("player data is a compound") };
    fields.retain(|(key, _)| key != "Pos");
    fields.push(("Pos".into(), Nbt::List(vec![Nbt::Double(f64::NAN), Nbt::Double(0.0), Nbt::Double(0.0)])));
    let mut loaded = ServerPlayer::new(Profile { uuid: 2, name: "Bob".into(), properties: Vec::new() }, ClientInfo::default(), Outbound::detached(), BlockPos::ZERO);
    let cause = loaded.load_nbt(&Nbt::Compound(fields)).unwrap_err();
    assert_eq!(cause.description, "java.lang.IllegalStateException: Entity has invalid position");
    let trace = crate::world::entity::entity_save::player_loading_exception(cause).print_stack_trace();
    assert!(trace.starts_with("net.minecraft.ReportedException: Loading entity NBT\n\tat net.minecraft.world.entity.Entity.load(Entity.java:2481)\n\tat java.base/java.util.Optional.ifPresent(Optional.java:178)\n"));
    assert!(trace.ends_with("Caused by: java.lang.IllegalStateException: Entity has invalid position\n\tat net.minecraft.world.entity.Entity.load(Entity.java:2440)\n\t... 12 more\n"));
}

#[test]
fn extra_fall_particles_are_spawned_once() {
    let mut level = testing::level();
    let player = testing::player(&mut level, ORIGIN, 0.0, 0.0);
    let (out, mut packets) = Outbound::capture();
    level.player_mut(player).connection = out;
    level.player_mut(player).spawn_extra_particles_on_fall = true;
    level.entity_mut(player).fall_distance = 1.5;
    let pos = BlockPos::new(0, -61, 0);
    let state = crate::world::level::BlockGetter::block_state(&mut level, pos);
    assert!(state.is_block(blocks::GRASS_BLOCK));
    level.check_fall_damage(player, -0.5, true, state, pos);
    assert!(!level.player(player).spawn_extra_particles_on_fall);
    let mut particles = Vec::new();
    while let Ok(packet) = packets.try_recv() {
        let mut r = Reader::new(&packet);
        if r.varint().unwrap() == clientbound::LEVEL_PARTICLES {
            let particle = r.varint().unwrap();
            let block = r.varint().unwrap();
            r.bool().unwrap();
            r.bool().unwrap();
            let pos = (r.f64().unwrap(), r.f64().unwrap(), r.f64().unwrap());
            let dist = (r.f32().unwrap(), r.f32().unwrap(), r.f32().unwrap());
            let speed = (r.f32().unwrap(), r.f32().unwrap(), r.f32().unwrap());
            particles.push((particle, block, pos, dist, speed, r.varint().unwrap()));
        }
    }
    let block_particle = wafer_data::registry_id(wafer_data::registry::PARTICLE_TYPE, "minecraft:block");
    assert_eq!(particles, [(block_particle, state.id() as i32, (0.5, -60.0, 0.5), (0.3, 0.3, 0.3), (0.15, 0.15, 0.15), 75)]);
}

#[test]
fn data_get_lists_ender_pearls_in_other_dimensions() {
    use crate::commands::arguments::selector::entity_world::EntityWorld;
    let mut server = testing::server();
    let alice = join(&mut server, "Alice");
    let nether = server.levels.iter().position(|level| level.dimension == crate::server::THE_NETHER).expect("a nether");
    summon_pearl(&mut server, nether, offline_uuid("Alice"), [0.5, 70.0, 0.5], [0.0, 0.0, 0.0]);
    server.flush_other_level_ender_pearls();
    assert_eq!(server.player(alice).ender_pearls.len(), 1);
    let Some(Nbt::List(pearls)) = server.entity_tag_to_compare(alice).get("ender_pearls").cloned() else { panic!("no ender pearls") };
    assert_eq!(pearls.len(), 1);
    assert_eq!(pearls[0].get("ender_pearl_dimension"), Some(&Nbt::String(crate::server::THE_NETHER.into())));
}
