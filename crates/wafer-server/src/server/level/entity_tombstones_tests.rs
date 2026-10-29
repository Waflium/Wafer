use wafer_core::Vec3;

use crate::server::level::{ServerLevel, testing};
use crate::world::difficulty::Difficulty;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::marker::Marker;
use crate::world::entity::mob::Mob;
use crate::world::entity::monster::breeze::breeze::Breeze;
use crate::world::entity::monster::hoglin::hoglin::Hoglin;
use crate::world::entity::monster::zombie::zombie::Zombie;
use crate::world::entity::{EntityId, LevelEntity};

fn add(level: &mut ServerLevel, mut mob: Mob, x: f64, z: f64) -> EntityId {
    mob.entity.snap_to(x, -60.0, z, 0.0, 0.0);
    mob.persistence_required = true;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn add_marker(level: &mut ServerLevel, x: f64, z: f64) -> EntityId {
    let mut marker = Marker::create();
    marker.entity.snap_to(x, -60.0, z, 0.0, 0.0);
    let id = marker.entity.id;
    level.add_fresh_entity(LevelEntity::Marker(marker));
    id
}

#[test]
fn removed_entities_stay_readable_until_buried_long_enough() {
    let mut level = testing::level();
    let zombie = add(&mut level, Zombie::create(), 0.5, 0.5);
    level.discard(zombie);
    level.tick();
    assert!(level.find_entity(zombie).is_none());
    assert!(level.entity(zombie).is_removed());
    assert_eq!(level.health(zombie), 20.0);
    for _ in 0..700 {
        level.tick();
    }
    assert!(level.tombstone(zombie).is_none());
    assert!(level.entity(zombie).is_removed());
    assert_eq!(level.health(zombie), 0.0);
}

#[test]
fn brain_targets_that_were_removed_are_forgotten_without_crashing() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let _player = testing::player(&mut level, Vec3::new(30.5, -60.0, 30.5), 0.0, 0.0);
    let breeze = add(&mut level, Breeze::create(), 0.5, 4.5);
    let hoglin = add(&mut level, Hoglin::create(), 4.5, 4.5);
    let victim = add(&mut level, Zombie::create(), 2.5, 2.5);
    level.tick();
    level.get_brain_mut(breeze).set_memory(memory_module_type::ATTACK_TARGET, victim);
    level.get_brain_mut(hoglin).set_memory(memory_module_type::ATTACK_TARGET, victim);
    level.tick();
    level.discard(victim);
    for _ in 0..1000 {
        level.tick();
    }
    assert_ne!(level.get_brain(breeze).get_memory(memory_module_type::ATTACK_TARGET), Some(&victim));
}

#[test]
fn mob_targets_that_were_removed_or_never_living_do_not_crash() {
    let mut level = testing::level();
    level.difficulty = Difficulty::Normal;
    let zombie = add(&mut level, Zombie::create(), 0.5, 0.5);
    let other = add(&mut level, Zombie::create(), 3.5, 0.5);
    let marker = add_marker(&mut level, 5.5, 0.5);
    level.mob_mut(zombie).expect("a mob").target = Some(other);
    level.tick();
    level.discard(other);
    for _ in 0..700 {
        level.tick();
    }
    level.mob_mut(zombie).expect("a mob").target = Some(marker);
    for _ in 0..20 {
        level.tick();
    }
    assert!(level.living(marker).hurt_time >= 0);
    assert_eq!(level.health(marker), 0.0);
}

#[test]
fn players_that_left_are_read_as_removed() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let zombie = add(&mut level, Zombie::create(), 2.5, 0.5);
    level.mob_mut(zombie).expect("a mob").target = Some(player);
    level.tick();
    level.remove_player(player);
    assert!(level.player(player).entity.is_removed());
    assert!(level.entity(player).is_removed());
    assert_eq!(level.health(player), 0.0);
    level.player_mut(player).health = 5.0;
    for _ in 0..40 {
        level.tick();
    }
}

#[test]
fn level_selectors_list_players_in_player_list_order_across_dimensions() {
    use crate::commands::arguments::selector::entity_world::{EntityWorld, LevelWorld};
    use crate::commands::perform_prefixed_command;
    use crate::commands::source::CommandSourceStack;
    use crate::network::login::Profile;
    use crate::server::level::lent_levels::LentLevels;
    let mut server = testing::server();
    let first = testing::server_player_with_profile(&mut server, Profile { uuid: 1, name: "First".into(), properties: Vec::new() }, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let second = testing::server_player_with_profile(&mut server, Profile { uuid: 2, name: "Second".into(), properties: Vec::new() }, Vec3::new(2.5, -60.0, 0.5), 0.0, 0.0);
    let third = testing::server_player_with_profile(&mut server, Profile { uuid: 3, name: "Third".into(), properties: Vec::new() }, Vec3::new(4.5, -60.0, 0.5), 0.0, 0.0);
    let console = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &console, "execute in minecraft:the_nether run tp First 0.5 64 0.5");
    let expected: Vec<EntityId> = EntityWorld::players(&server).map(|player| player.entity.id).collect();
    assert_eq!(expected, vec![first, second, third]);
    let mut lent = LentLevels::lend(&mut server.levels, 0);
    let level = lent.level();
    let listed: Vec<EntityId> = LevelWorld(level).players().map(|player| player.entity.id).collect();
    assert_eq!(listed, expected);
}

#[test]
fn a_panic_while_ticking_an_entity_becomes_a_ticking_entity_crash_report() {
    let mut level = testing::level();
    let zombie = add(&mut level, Zombie::create(), 0.5, 0.5);
    let rider = add(&mut level, Zombie::create(), 0.5, 0.5);
    assert!(level.start_riding(rider, zombie, true, false));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| level.guard_entity_tick(zombie, |_| panic!("boom")))).unwrap_err();
    let reported = panic.downcast::<crate::crash_report::ReportedException>().unwrap();
    assert_eq!(reported.0.title(), "Ticking entity");
    assert_eq!(reported.0.exception(), "boom");
    let report = reported.0.get_friendly_report(&crate::crash_report::ReportType::CRASH);
    assert!(report.contains("-- Entity being ticked --"));
    assert!(report.contains("Entity Type: minecraft:zombie (net.minecraft.world.entity.monster.zombie.Zombie)"));
    assert!(report.contains("Entity Name: Zombie"));
    let name = level.level_name.clone();
    assert!(report.contains(&format!("Entity's Passengers: [Zombie['Zombie'/{rider}, l='ServerLevel[{name}]', x=0.50, y=-60.00, z=0.50]]")), "{report}");
    assert!(report.contains("Entity's Vehicle: null"));
}

#[test]
fn entity_strings_round_half_up_and_show_the_removal_reason() {
    let mut level = testing::level();
    let zombie = add(&mut level, Zombie::create(), 0.125, 0.375);
    level.entity_mut(zombie).removal = Some(crate::world::entity::RemovalReason::Killed);
    let name = level.level_name.clone();
    assert_eq!(level.entity_to_string(zombie), format!("Zombie['Zombie'/{zombie}, l='ServerLevel[{name}]', x=0.13, y=-60.00, z=0.38, removed=KILLED]"));
}
