use wafer_core::{ChunkPos, Vec3};

use crate::commands::perform_prefixed_command;
use crate::commands::source::CommandSourceStack;
use crate::server::Server;
use crate::server::level::testing;
use crate::world::entity::EntityId;

fn server() -> Server {
    let mut server = testing::server();
    for index in 0..3 {
        for x in -2..=3 {
            for z in -2..=2 {
                server.levels[index].chunks.chunk(ChunkPos::new(x, z));
            }
        }
    }
    run(&mut server, "forceload add -16 -16 63 31");
    run(&mut server, "execute in minecraft:the_nether run forceload add -16 -16 63 31");
    run(&mut server, "execute in minecraft:the_end run forceload add -16 -16 63 31");
    for index in 0..3 {
        server.levels[index].finish_chunk_work();
    }
    server
}

fn nether(server: &Server) -> usize {
    server.get_level("minecraft:the_nether").unwrap()
}

fn end(server: &Server) -> usize {
    server.get_level("minecraft:the_end").unwrap()
}

fn run(server: &mut Server, command: &str) -> Vec<String> {
    server.console_log.borrow_mut().clear();
    let source = CommandSourceStack::server(server);
    perform_prefixed_command(server, &source, command);
    server.console_log.borrow().clone()
}

fn tick(server: &mut Server, count: usize) {
    for _ in 0..count {
        server.tick();
    }
}

fn entity_with_uuid(server: &Server, last: i32) -> Option<(usize, EntityId)> {
    let uuid = (1u128 << 96) | (2u128 << 64) | (3u128 << 32) | last as u32 as u128;
    server.levels.iter().enumerate().find_map(|(index, level)| level.entity_manager.visible_entity_storage.get(uuid).map(|id| (index, id)))
}

fn nether_portal(server: &mut Server) {
    run(server, "fill 0 -61 0 3 -57 0 obsidian");
    run(server, "fill 1 -60 0 2 -58 0 nether_portal[axis=x]");
}

#[test]
fn items_in_a_nether_portal_arrive_in_the_nether_on_the_tick_after_they_touch_it() {
    let mut server = server();
    nether_portal(&mut server);
    run(&mut server, "summon item 1.5 -60 0.5 {UUID:[I;1,2,3,1],Item:{id:\"minecraft:stone\",count:5},PickupDelay:50s}");
    tick(&mut server, 1);
    assert_eq!(entity_with_uuid(&server, 1).map(|(level, _)| level), Some(0));
    tick(&mut server, 1);
    let (level, id) = entity_with_uuid(&server, 1).unwrap();
    assert_eq!(level, nether(&server));
    let arrived = server.levels[nether(&server)].entity(id);
    let moved_back_into_the_portal = (arrived.tick_count + id) % 4 == 0;
    assert_eq!(arrived.portal_cooldown, if moved_back_into_the_portal { 300 } else { 299 });
    assert!(!server.levels[0].tick_suspended());
    assert!(!server.levels[0].tick_continuation.is_suspended());
}

#[test]
fn the_old_entity_finishes_its_tick_after_the_teleport_like_vanilla() {
    let mut server = server();
    nether_portal(&mut server);
    run(&mut server, "setblock 1 -60 3 glowstone");
    run(&mut server, "summon tnt 1.5 -60 0.5 {UUID:[I;1,2,3,1],fuse:2s}");
    tick(&mut server, 1);
    assert_eq!(run(&mut server, "execute if block 1 -60 3 glowstone"), vec!["Test passed".to_string()]);
    tick(&mut server, 1);
    assert_eq!(run(&mut server, "execute if block 1 -60 3 glowstone"), vec!["Test failed".to_string()]);
    let out = run(&mut server, "execute in minecraft:the_nether if entity @e[type=tnt]");
    assert!(out.is_empty() || out[0].starts_with("Test"), "{out:?}");
}

#[test]
fn vehicles_arrive_with_their_passengers_and_both_leave_the_old_level() {
    let mut server = server();
    nether_portal(&mut server);
    run(&mut server, "summon minecart 1.5 -60 0.5 {UUID:[I;1,2,3,1],Silent:1b}");
    run(&mut server, "summon zombie 1.5 -60 0.5 {UUID:[I;1,2,3,2],NoAI:1b,PersistenceRequired:1b,Silent:1b}");
    run(&mut server, "ride 00000001-0000-0002-0000-000300000002 mount 00000001-0000-0002-0000-000300000001");
    tick(&mut server, 2);
    let (cart_level, cart) = entity_with_uuid(&server, 1).unwrap();
    let (zombie_level, zombie) = entity_with_uuid(&server, 2).unwrap();
    assert_eq!((cart_level, zombie_level), (nether(&server), nether(&server)));
    assert_eq!(server.levels[nether(&server)].vehicle(zombie), Some(cart));
    assert!(server.levels[0].entities.values().all(|entity| entity.entity().uuid & 0xffff_ffff > 2 || entity.entity().is_removed()));
}

#[test]
fn mobs_that_changed_dimension_arrive_with_their_equipment() {
    let mut server = server();
    nether_portal(&mut server);
    run(&mut server, "summon zombie 1.5 -60 0.5 {UUID:[I;1,2,3,1],NoAI:1b,PersistenceRequired:1b,Silent:1b,equipment:{head:{id:\"minecraft:iron_helmet\",count:1}}}");
    tick(&mut server, 2);
    let (level, id) = entity_with_uuid(&server, 1).unwrap();
    assert_eq!(level, nether(&server));
    assert!(server.levels[nether(&server)].item_by_slot(id, wafer_data::EquipmentSlot::Head).is_some());
}

#[test]
fn the_old_body_of_a_mob_that_changed_dimension_is_removed_but_empty_handed_until_the_tick_ends() {
    let mut level = testing::level();
    let mut zombie = crate::world::entity::monster::zombie::zombie::Zombie::create();
    zombie.equipment[wafer_data::EquipmentSlot::Head as usize] = Some(crate::world::item::ItemStack::new(wafer_data::items::IRON_HELMET, 1));
    let id = zombie.entity.id;
    level.add_fresh_entity(crate::world::entity::LevelEntity::Mob(Box::new(zombie)));
    level.remove_after_changing_dimensions(id);
    assert_eq!(level.entity(id).removal, Some(crate::world::entity::RemovalReason::ChangedDimension));
    assert!(level.find_entity(id).is_some());
    assert!(level.item_by_slot(id, wafer_data::EquipmentSlot::Head).is_none());
}

#[test]
fn falling_blocks_leave_the_overworld_through_end_portals() {
    let mut server = server();
    run(&mut server, "fill 0 -61 0 2 -61 2 end_portal");
    run(&mut server, "summon falling_block 1.5 -60.9 1.5 {UUID:[I;1,2,3,1],BlockState:{Name:\"minecraft:sand\"},Time:1}");
    tick(&mut server, 2);
    assert_ne!(entity_with_uuid(&server, 1).map(|(level, _)| level), Some(0));
}

#[test]
fn thrown_ender_pearls_teleport_owners_in_other_levels_and_are_discarded_afterwards() {
    let mut server = server();
    let player = testing::server_player(&mut server, Vec3::new(20.5, -60.0, 20.5), 0.0, 0.0);
    run(&mut server, "execute in minecraft:the_nether run summon ender_pearl 10.5 90 10.5 {UUID:[I;1,2,3,1],Motion:[0.0d,-0.5d,0.0d]}");
    run(&mut server, "execute as Tester run data modify entity 00000001-0000-0002-0000-000300000001 Owner set from entity @s UUID");
    for _ in 0..80 {
        server.tick();
        if server.level_of(player) == Some(nether(&server)) {
            break;
        }
    }
    assert_eq!(server.level_of(player), Some(nether(&server)));
    tick(&mut server, 1);
    assert!(entity_with_uuid(&server, 1).is_none());
}

#[test]
fn survival_players_in_nether_portals_continue_their_tick_in_the_nether() {
    let mut server = server();
    nether_portal(&mut server);
    let player = testing::server_player(&mut server, Vec3::new(1.5, -60.0, 0.5), 0.0, 0.0);
    run(&mut server, "gamemode creative Tester");
    tick(&mut server, 3);
    assert_eq!(server.level_of(player), Some(nether(&server)));
    let tick_count = server.levels[nether(&server)].entity(player).tick_count;
    assert!(tick_count >= 2, "{tick_count}");
}

#[test]
fn command_sources_keep_naming_the_entity_they_run_as_after_it_is_gone() {
    let mut server = server();
    run(&mut server, "summon cow 4.5 -60 4.5 {UUID:[I;1,2,3,1],NoAI:1b}");
    let (level, cow) = entity_with_uuid(&server, 1).unwrap();
    let source = CommandSourceStack::server(&server).with_entity(cow);
    server.levels[level].discard(cow);
    server.levels[level].remove_entity(cow);
    assert!(server.level_of(cow).is_none());
    assert_eq!(source.text_name(&server), "Cow");
    assert_eq!(source.display_name(&server).get_string(), "Cow");
}
