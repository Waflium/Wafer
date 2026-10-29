use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, items};
use wafer_protocol::nbt::Nbt;

use super::wolf::{self, Wolf};
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::entity::ai::goal::target::nearest_attackable_target_goal::TargetType;
use crate::world::entity::spawn_group_data::SpawnGroupData;
use crate::world::entity::tamable_animal;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::dye_color::DyeColor;
use crate::world::{InteractionHand, InteractionResult};

fn setup() -> (ServerLevel, EntityId, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = Wolf::create();
    mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.0));
    let wolf = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    (level, player, wolf)
}

fn tamed() -> (ServerLevel, EntityId, EntityId) {
    let (mut level, player, wolf) = setup();
    level.tame(wolf, player);
    (level, player, wolf)
}

fn interact(level: &mut ServerLevel, player: EntityId, wolf: EntityId, held: ItemStack) -> InteractionResult {
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(held));
    let location = level.entity(wolf).position();
    level.interact_on(player, wolf, InteractionHand::MainHand, location)
}

fn held(level: &ServerLevel, player: EntityId) -> Option<ItemStack> {
    level.player(player).item_in_hand(InteractionHand::MainHand).filter(|stack| !stack.is_empty())
}

#[test]
fn bones_tame_wild_wolves_one_time_in_three_and_are_used_up() {
    let (mut level, player, wolf) = setup();
    let mut tries = 0;
    while !level.is_tame(wolf) {
        tries += 1;
        assert!(tries < 100, "the wolf never accepted a bone");
        assert_eq!(interact(&mut level, player, wolf, ItemStack::new(items::BONE, 1)), InteractionResult::SUCCESS_SERVER);
        assert!(held(&level, player).is_none());
    }
    assert_eq!(level.get_owner(wolf), Some(player));
    assert!(level.is_ordered_to_sit(wolf));
    assert_eq!(level.health(wolf), 40.0);
    assert_eq!(level.max_health(wolf), 40.0);
    assert_eq!(interact(&mut level, player, wolf, ItemStack::new(items::BONE, 1)), InteractionResult::SUCCESS.without_item());
    assert!(!level.is_ordered_to_sit(wolf));
    assert_eq!(held(&level, player).map(|stack| stack.count), Some(1));
}

#[test]
fn angry_wolves_refuse_bones() {
    let (mut level, player, wolf) = setup();
    level.set_time_to_remain_angry(wolf, 400);
    assert_eq!(interact(&mut level, player, wolf, ItemStack::new(items::BONE, 1)), InteractionResult::Pass);
    assert_eq!(held(&level, player).map(|stack| stack.count), Some(1));
}

#[test]
fn owners_dye_collars_once_per_color() {
    let (mut level, player, wolf) = tamed();
    assert_eq!(wolf::get_collar_color(level.entity(wolf)), DyeColor::VALUES[14]);
    assert_eq!(interact(&mut level, player, wolf, ItemStack::new(items::BLUE_DYE, 2)), InteractionResult::SUCCESS);
    assert_eq!(wolf::get_collar_color(level.entity(wolf)).name, "blue");
    assert_eq!(held(&level, player).map(|stack| stack.count), Some(1));
    let sitting = level.is_ordered_to_sit(wolf);
    assert_eq!(interact(&mut level, player, wolf, ItemStack::new(items::BLUE_DYE, 1)), InteractionResult::Pass);
    assert_eq!(level.is_ordered_to_sit(wolf), sitting);
    assert_eq!(held(&level, player).map(|stack| stack.count), Some(1));
}

#[test]
fn feeding_heals_by_twice_the_nutrition() {
    let (mut level, player, wolf) = tamed();
    level.set_health(wolf, 20.0);
    assert_eq!(interact(&mut level, player, wolf, ItemStack::new(items::BEEF, 2)), InteractionResult::SUCCESS);
    assert_eq!(level.health(wolf), 26.0);
    assert_eq!(held(&level, player).map(|stack| stack.count), Some(1));
}

#[test]
fn wolf_armor_absorbs_damage_and_is_repaired_with_scutes() {
    let (mut level, player, wolf) = tamed();
    assert_eq!(interact(&mut level, player, wolf, ItemStack::new(items::WOLF_ARMOR, 1)), InteractionResult::SUCCESS);
    assert!(held(&level, player).is_none());
    assert!(level.is_wearing_body_armor(wolf));
    level.hurt_server(wolf, DamageSource::new(wafer_data::damage_types::GENERIC), 4.5);
    assert_eq!(level.health(wolf), 40.0);
    assert_eq!(level.get_body_armor_item(wolf).damage_value(), 5);
    level.set_ordered_to_sit(wolf, true);
    level.set_in_sitting_pose(wolf, true);
    assert_eq!(interact(&mut level, player, wolf, ItemStack::new(items::ARMADILLO_SCUTE, 1)), InteractionResult::SUCCESS);
    assert_eq!(level.get_body_armor_item(wolf).damage_value(), 0);
    level.living_mut(wolf).damage_cooldown_time = 0;
    level.hurt_server(wolf, DamageSource::new(wafer_data::damage_types::MAGIC), 2.0);
    assert_eq!(level.health(wolf), 38.0);
    assert_eq!(level.get_body_armor_item(wolf).damage_value(), 0);
}

#[test]
fn only_owners_shear_off_wolf_armor() {
    let (mut level, player, wolf) = tamed();
    level.set_item_slot(wolf, EquipmentSlot::Body, ItemStack::new(items::WOLF_ARMOR, 1));
    let stranger = testing::player_with_profile(
        &mut level,
        crate::network::login::Profile { uuid: 2, name: "Stranger".into(), properties: Vec::new() },
        Vec3::new(1.5, -60.0, 0.5),
        0.0,
        0.0,
    );
    interact(&mut level, stranger, wolf, ItemStack::new(items::SHEARS, 1));
    assert!(level.is_wearing_body_armor(wolf));
    interact(&mut level, player, wolf, ItemStack::new(items::SHEARS, 1));
    assert!(!level.is_wearing_body_armor(wolf));
}

#[test]
fn tamed_wolves_save_and_load_like_vanilla() {
    let (mut level, player, wolf) = tamed();
    wolf::set_collar_color(level.entity_mut(wolf), DyeColor::VALUES[11]);
    level.set_ordered_to_sit(wolf, true);
    level.set_time_to_remain_angry(wolf, 100);
    let mut out = Vec::new();
    level.save_without_id(wolf, &mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("CollarColor"), Some(Nbt::Byte(11)));
    assert_eq!(get("variant"), Some(Nbt::String("minecraft:pale".into())));
    assert_eq!(get("sound_variant"), Some(Nbt::String("minecraft:classic".into())));
    assert_eq!(get("Sitting"), Some(Nbt::Byte(1)));
    assert_eq!(get("anger_end_time"), Some(Nbt::Long(level.game_time + 100)));
    assert!(get("Owner").is_some());
    let mut loaded = ServerLevel::create_entity("minecraft:wolf").unwrap();
    loaded.load(&Nbt::Compound(out.clone())).unwrap();
    let LevelEntity::Mob(mob) = &loaded else { unreachable!() };
    assert!(tamable_animal::is_tame(&mob.entity));
    assert!(tamable_animal::is_in_sitting_pose(&mob.entity));
    assert_eq!(tamable_animal::get_owner_reference(&mob.entity).map(|owner| owner.get_uuid()), Some(level.entity(player).uuid));
    assert_eq!(wolf::get_collar_color(&mob.entity).id, 11);
    assert_eq!(mob.health, 40.0);
}

#[test]
fn old_owner_names_are_converted_when_loading() {
    use crate::network::login::offline_uuid;
    use crate::server::players::NameAndId;
    use crate::server::players::cached_user_name_to_id_resolver::CachedUserNameToIdResolver;
    use crate::server::players::old_users_converter::UserLookup;
    use crate::server::services::Services;
    let mut level = testing::level();
    let cache = std::sync::Arc::new(CachedUserNameToIdResolver::load(None));
    cache.add(NameAndId { id: 7, name: "Rozqn".into() });
    level.user_lookup = Some(UserLookup { name_to_id_cache: cache, services: Services::offline(), uses_authentication: false });
    let mut load = |owner: &str| {
        let tag = Nbt::Compound(vec![("id".into(), Nbt::String("minecraft:wolf".into())), ("Owner".into(), Nbt::String(owner.into()))]);
        let Some(LevelEntity::Mob(mob)) = level.load_entity(&tag) else { unreachable!() };
        (tamable_animal::is_tame(&mob.entity), tamable_animal::get_owner_reference(&mob.entity).map(|owner| owner.get_uuid()))
    };
    assert_eq!(load("ROZQN"), (true, Some(7)));
    assert_eq!(load("Nobody"), (true, Some(offline_uuid("nobody"))));
    assert_eq!(load(""), (false, None));
    assert_eq!(load("a name that is far too long"), (false, None));
    assert_eq!(load("12345678-1234-1234-1234-123456789abc"), (true, Some(0x12345678_1234_1234_1234_123456789abc)));
}

#[test]
fn bred_puppies_of_tamed_wolves_belong_to_the_owner() {
    let (mut level, player, wolf) = tamed();
    let mut other = Wolf::create();
    other.entity.set_pos(Vec3::new(1.5, -60.0, 2.0));
    let partner = other.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(other)));
    level.tame(partner, player);
    assert!(level.can_mate(wolf, partner) == level.is_in_love(wolf));
    level.set_in_love(wolf, Some(player));
    level.set_in_love(partner, Some(player));
    assert!(level.can_mate(wolf, partner));
    level.set_in_sitting_pose(partner, true);
    assert!(!level.can_mate(wolf, partner));
    level.set_in_sitting_pose(partner, false);
    let puppy = level.get_breed_offspring(wolf, partner).unwrap();
    assert!(tamable_animal::is_tame(&puppy.entity));
    assert_eq!(tamable_animal::get_owner_reference(&puppy.entity).map(|owner| owner.get_uuid()), Some(level.entity(player).uuid));
    assert_eq!(puppy.health, 40.0);
}

#[test]
fn wolves_spare_their_owners_other_pets_and_creepers() {
    let (mut level, player, wolf) = tamed();
    let mut other = Wolf::create();
    other.entity.set_pos(Vec3::new(3.5, -60.0, 2.0));
    let pet = other.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(other)));
    assert!(level.wolf_wants_to_attack(pet, player));
    level.tame(pet, player);
    assert!(!level.wolf_wants_to_attack(pet, player));
    assert!(!level.can_attack(wolf, player));
    assert!(level.is_allied_to(wolf, player));
    let creeper = ServerLevel::create_entity("minecraft:creeper").unwrap();
    let creeper_id = creeper.entity().id;
    level.add_fresh_entity(creeper);
    assert!(!level.wolf_wants_to_attack(creeper_id, player));
}

#[test]
fn packs_share_their_variant_and_have_no_puppies() {
    let mut level = testing::level();
    let mut group_data = None;
    for _ in 0..8 {
        let mut mob = Wolf::create();
        mob.entity.set_pos(Vec3::new(0.5, -60.0, 0.5));
        let difficulty = level.current_difficulty_at(mob.entity.block_position());
        let (entity, next) =
            level.finalize_spawn_detached(LevelEntity::Mob(Box::new(mob)), &difficulty, crate::world::entity::entity_spawn_reason::EntitySpawnReason::Natural, group_data);
        group_data = next;
        let LevelEntity::Mob(mob) = entity else { unreachable!() };
        assert!(!mob.is_baby());
        assert_eq!(crate::world::entity::variant::variant_utils::variant_name(wolf::VARIANT_REGISTRY, wolf::get_variant(&mob.entity)), "minecraft:pale");
    }
    let Some(SpawnGroupData::WolfPack(pack)) = group_data else { panic!("no pack data") };
    assert_eq!(pack.ageable_mob_group_data.get_group_size(), 8);
}

#[test]
fn sitting_wolves_teleport_only_when_told_to_stand() {
    let (mut level, player, wolf) = tamed();
    level.set_ordered_to_sit(wolf, true);
    level.entity_mut(player).set_pos(Vec3::new(20.5, -60.0, 0.5));
    for _ in 0..20 {
        level.tick();
    }
    assert!(level.entity(wolf).position().distance_to_sqr(Vec3::new(0.5, -60.0, 2.0)) < 1.0);
    level.set_ordered_to_sit(wolf, false);
    for _ in 0..20 {
        level.tick();
    }
    let distance = level.entity(wolf).distance_to_sqr(level.entity(player));
    assert!(distance < 25.0, "the wolf stayed {distance} away");
}

#[test]
fn wolves_hunt_skeletons_and_skeletons_flee() {
    let (mut level, _player, wolf) = setup();
    let mut skeleton = ServerLevel::create_entity("minecraft:skeleton").unwrap();
    skeleton.entity_mut().set_pos(Vec3::new(4.5, -60.0, 2.0));
    let skeleton_id = skeleton.entity().id;
    level.add_fresh_entity(skeleton);
    let mut targeted = false;
    let mut fled = false;
    for _ in 0..100 {
        level.tick();
        targeted |= level.get_target(wolf) == Some(skeleton_id);
        fled |= level.mob(skeleton_id).is_some_and(|mob| {
            mob.ai.goal_selector.get_available_goals().iter().any(|goal| goal.is_running() && matches!(goal.get_goal(), crate::world::entity::ai::goal::goal::Goal::AvoidEntity(_)))
        });
    }
    assert!(targeted, "the wolf never went for the skeleton");
    assert!(fled, "the skeleton never avoided the wolf");
}

#[test]
fn wild_wolves_hunt_sheep_but_not_cows() {
    let selector = wolf::prey_selector;
    let (mut level, _player, wolf) = setup();
    let sheep = ServerLevel::create_entity("minecraft:sheep").unwrap();
    let cow = ServerLevel::create_entity("minecraft:cow").unwrap();
    let (sheep_id, cow_id) = (sheep.entity().id, cow.entity().id);
    level.add_fresh_entity(sheep);
    level.add_fresh_entity(cow);
    assert!(TargetType::Animal.is_instance(&level, sheep_id) && selector(wolf, sheep_id, &level));
    assert!(TargetType::Animal.is_instance(&level, cow_id) && !selector(wolf, cow_id, &level));
}

#[test]
fn wolves_follow_and_report_to_owners_in_another_dimension() {
    use crate::commands::perform_prefixed_command;
    use crate::commands::source::CommandSourceStack;
    use crate::network::connection::Outbound;
    use crate::network::login::Profile;
    use crate::server::level::lent_levels::LentLevels;
    use wafer_data::packets::play::clientbound;
    use wafer_protocol::Reader;
    let mut server = testing::server();
    let player = testing::server_player_with_profile(&mut server, Profile { uuid: 1, name: "Owner".into(), properties: Vec::new() }, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    server.overworld_mut().handle_player_loaded(player);
    let mut mob = Wolf::create();
    mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.0));
    let wolf = mob.entity.id;
    server.overworld_mut().add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    server.overworld_mut().tame(wolf, player);
    let console = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &console, "forceload add -16 -16 47 15");
    perform_prefixed_command(&mut server, &console, "fill 10 63 -6 30 63 6 stone");
    perform_prefixed_command(&mut server, &console, "execute in minecraft:the_nether run tp Owner 20.5 64 0.5");
    let nether = server.level_of(player).unwrap();
    assert_ne!(nether, 0);
    for _ in 0..40 {
        server.tick();
    }
    let position = server.overworld().entity(wolf).position();
    assert!(position.distance_to_sqr(Vec3::new(20.5, 64.0, 0.5)) < 25.0, "the wolf stayed at {position:?}");
    let (out, mut packets) = Outbound::capture();
    server.levels[nether].player_mut(player).connection = out;
    LentLevels::lend(&mut server.levels, 0).level().kill(wolf);
    let ids: Vec<i32> = std::iter::from_fn(|| packets.try_recv().ok()).map(|p| Reader::new(&p).varint().unwrap()).collect();
    assert!(ids.contains(&clientbound::SYSTEM_CHAT), "{ids:?}");
}

#[test]
fn killing_a_wolf_by_command_reports_to_its_owner_in_another_dimension() {
    use crate::commands::perform_prefixed_command;
    use crate::commands::source::CommandSourceStack;
    use crate::network::connection::Outbound;
    use crate::network::login::Profile;
    use wafer_data::packets::play::clientbound;
    use wafer_protocol::Reader;
    let mut server = testing::server();
    let player = testing::server_player_with_profile(&mut server, Profile { uuid: 1, name: "Owner".into(), properties: Vec::new() }, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    server.overworld_mut().handle_player_loaded(player);
    let mut mob = Wolf::create();
    mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.0));
    let wolf = mob.entity.id;
    server.overworld_mut().add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    server.overworld_mut().tame(wolf, player);
    let console = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &console, "execute in minecraft:the_nether run tp Owner 20.5 64 0.5");
    let nether = server.level_of(player).unwrap();
    assert_ne!(nether, 0);
    let (out, mut packets) = Outbound::capture();
    server.levels[nether].player_mut(player).connection = out;
    perform_prefixed_command(&mut server, &console, "kill @e[type=minecraft:wolf]");
    let ids: Vec<i32> = std::iter::from_fn(|| packets.try_recv().ok()).map(|p| Reader::new(&p).varint().unwrap()).collect();
    assert!(ids.contains(&clientbound::SYSTEM_CHAT), "{ids:?}");
}

#[test]
fn selectors_give_tamed_wolves_the_team_of_an_owner_in_another_dimension() {
    use crate::commands::perform_prefixed_command;
    use crate::commands::source::CommandSourceStack;
    use crate::network::login::Profile;
    let mut server = testing::server();
    let player = testing::server_player_with_profile(&mut server, Profile { uuid: 1, name: "Owner".into(), properties: Vec::new() }, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    server.overworld_mut().handle_player_loaded(player);
    let mut mob = Wolf::create();
    mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.0));
    let wolf = mob.entity.id;
    server.overworld_mut().add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    server.overworld_mut().tame(wolf, player);
    let console = CommandSourceStack::server(&server);
    perform_prefixed_command(&mut server, &console, "team add red");
    perform_prefixed_command(&mut server, &console, "team join red Owner");
    perform_prefixed_command(&mut server, &console, "execute in minecraft:the_nether run tp Owner 20.5 64 0.5");
    assert_ne!(server.level_of(player), Some(0));
    perform_prefixed_command(&mut server, &console, "tag @e[type=minecraft:wolf,team=red] add follows_owner");
    assert!(server.overworld().entity(wolf).tags.iter().any(|tag| tag == "follows_owner"));
}
