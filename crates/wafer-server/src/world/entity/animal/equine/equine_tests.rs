use wafer_core::Vec3;
use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_data::{EquipmentSlot, items};

use super::abstract_chested_horse;
use super::abstract_horse::{self, create_offspring_attribute};
use super::donkey::Donkey;
use super::horse::{self, Horse};
use super::markings::Markings;
use super::variant::Variant;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::inventory::abstract_container_menu::MenuKind;
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

fn setup(mob: Mob) -> (ServerLevel, EntityId, EntityId) {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = mob;
    mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.5));
    let horse = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    (level, player, horse)
}

fn interact(level: &mut ServerLevel, player: EntityId, horse: EntityId, held: Option<ItemStack>) -> InteractionResult {
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, held);
    let location = level.entity(horse).position();
    level.interact_on(player, horse, InteractionHand::MainHand, location)
}

fn held_count(level: &ServerLevel, player: EntityId) -> i32 {
    level.player(player).item_in_hand(InteractionHand::MainHand).map_or(0, |stack| stack.count)
}

fn items_on_ground(level: &ServerLevel) -> Vec<(wafer_data::Item, i32)> {
    level.entities.values().filter_map(|e| e.as_item()).map(|item| (item.item.get_item(), item.item.count)).collect()
}

#[test]
fn attribute_generators_span_vanillas_ranges() {
    assert_eq!(abstract_horse::generate_max_health(|_| 0), 15.0);
    assert_eq!(abstract_horse::generate_max_health(|i| i - 1), 30.0);
    assert_eq!(abstract_horse::generate_jump_strength(|| 0.0), 0.4f32 as f64);
    assert_eq!(abstract_horse::generate_jump_strength(|| 1.0), 0.4f32 as f64 + 0.2 + 0.2 + 0.2);
    assert_eq!(abstract_horse::generate_speed(|| 0.0), 0.45f32 as f64 * 0.25);
    assert_eq!(abstract_horse::generate_speed(|| 1.0), (0.45f32 as f64 + 0.3 + 0.3 + 0.3) * 0.25);
}

#[test]
fn attributes_match_vanillas_seeded_values() {
    let mut random = LegacyRandomSource::new(42);
    let cases = [(20.0, 25.0, 15.0, 30.0), (30.0, 30.0, 15.0, 30.0), (15.0, 15.0, 15.0, 30.0), (0.1125, 0.3375, 0.1125, 0.3375), (0.4, 1.0, 0.4, 1.0)];
    let expected: [u64; 5] = [4627222059112358746, 4628991381594437840, 4624964726082754410, 4598846710794173620, 4604044323178181929];
    for ((a, b, min, max), expected) in cases.into_iter().zip(expected) {
        assert_eq!(create_offspring_attribute(a, b, min, max, &mut random).to_bits(), expected);
    }
    let mut random = LegacyRandomSource::new(7);
    let expected: [(u32, u64, u64); 3] = [
        (1102053376, 4598393315016533283, 4603905289893420879),
        (1103626240, 4597689504884610876, 4605104822296931076),
        (1102053376, 4596932078048857670, 4604089482779784859),
    ];
    for (health, speed, jump) in expected {
        assert_eq!(abstract_horse::generate_max_health(|bound| random.next_int_bounded(bound)).to_bits(), health);
        assert_eq!(abstract_horse::generate_speed(|| random.next_double()).to_bits(), speed);
        assert_eq!(abstract_horse::generate_jump_strength(|| random.next_double()).to_bits(), jump);
    }
}

#[test]
fn variant_and_markings_pack_into_one_int() {
    let mut mob = Horse::create();
    horse::set_variant_and_markings(&mut mob.entity, Variant::DarkBrown, Markings::BlackDots);
    assert_eq!(horse::get_type_variant(&mob.entity), 6 | 4 << 8);
    horse::set_variant(&mut mob.entity, Variant::Gray);
    assert_eq!(horse::get_variant(&mob.entity), Variant::Gray);
    assert_eq!(horse::get_markings(&mob.entity), Markings::BlackDots);
}

#[test]
fn empty_hands_mount_wild_horses_until_they_buck_or_accept() {
    let (mut level, player, horse) = setup(Horse::create());
    assert_eq!(interact(&mut level, player, horse, None), InteractionResult::SUCCESS);
    assert_eq!(level.vehicle(player), Some(horse));
    assert_eq!(level.controlling_passenger(horse), None);
    let mut ticks = 0;
    while level.vehicle(player) == Some(horse) && !level.is_tamed(horse) {
        level.tick();
        ticks += 1;
        assert!(ticks < 2000, "the horse never reacted to its rider");
    }
    if level.is_tamed(horse) {
        assert_eq!(level.get_owner(horse), Some(player));
    } else {
        assert_eq!(level.get_temper(horse), 5);
        assert!(level.is_standing(horse));
    }
}

#[test]
fn feeding_heals_and_raises_temper() {
    let (mut level, player, horse) = setup(Horse::create());
    level.mob_mut(horse).unwrap().set_health(10.0);
    assert_eq!(interact(&mut level, player, horse, Some(ItemStack::new(items::WHEAT, 2))), InteractionResult::SUCCESS_SERVER);
    assert_eq!(level.health(horse), 12.0);
    assert_eq!(level.get_temper(horse), 3);
    assert_eq!(held_count(&level, player), 1);
    assert_eq!(interact(&mut level, player, horse, Some(ItemStack::new(items::STICK, 1))), InteractionResult::SUCCESS);
    assert!(level.is_standing(horse));
    assert_eq!(level.vehicle(player), None);
}

#[test]
fn saddled_tamed_horses_are_controlled_by_their_rider() {
    let (mut level, player, horse) = setup(Horse::create());
    level.tame_with_name(horse, player);
    assert_eq!(interact(&mut level, player, horse, Some(ItemStack::new(items::SADDLE, 1))), InteractionResult::SUCCESS);
    assert!(level.is_saddled(horse));
    assert_eq!(held_count(&level, player), 0);
    assert_eq!(interact(&mut level, player, horse, None), InteractionResult::SUCCESS);
    assert_eq!(level.controlling_passenger(horse), Some(player));
    assert!(!level.is_effective_ai(horse));
    assert!(!level.can_simulate_movement(horse));
    let position = level.entity(horse).position();
    for _ in 0..20 {
        level.tick();
    }
    assert_eq!(level.entity(horse).position(), position);
    assert_eq!(level.entity(horse).delta_movement, Vec3::ZERO);
    level.handle_player_loaded(player);
    level.handle_player_command(player, 3, 50);
    assert!(level.is_standing(horse));
}

#[test]
fn ridden_horses_take_fall_damage_from_client_moves() {
    let mut server = testing::server();
    let player = testing::server_player(&mut server, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let level = &mut server.levels[0];
    level.handle_player_loaded(player);
    let mut mob = Horse::create();
    mob.entity.set_pos(Vec3::new(0.5, -60.0, 2.5));
    let horse = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    level.tame_with_name(horse, player);
    level.set_item_slot(horse, EquipmentSlot::Saddle, ItemStack::new(items::SADDLE, 1));
    level.start_riding(player, horse, true, true);
    level.player_mut(player).movement.awaiting_position_from_client = None;
    level.tick_player(player);
    for i in 1..=10 {
        level.handle_move_vehicle(player, Vec3::new(0.5, -60.0 + i as f64 * 1.2, 2.5), 0.0, 0.0, false);
        level.tick();
        level.tick_player(player);
    }
    assert_eq!(level.entity(horse).y(), -48.0);
    for i in 1..=6 {
        level.handle_move_vehicle(player, Vec3::new(0.5, -48.0 - i as f64 * 2.0, 2.5), 0.0, 0.0, i == 6);
        level.tick();
        level.tick_player(player);
    }
    assert_eq!(level.entity(horse).y(), -60.0);
    assert_eq!(level.entity(horse).fall_distance, 0.0);
    assert_eq!(level.health(horse), 50.0);
}

#[test]
fn sneaking_owners_open_the_horse_inventory() {
    let (mut level, player, donkey) = setup(Donkey::create());
    level.tame_with_name(donkey, player);
    assert_eq!(interact(&mut level, player, donkey, Some(ItemStack::new(items::CHEST, 1))), InteractionResult::SUCCESS);
    assert!(level.has_chest(donkey));
    assert_eq!(level.mob(donkey).unwrap().abstract_horse().unwrap().inventory.items.len(), 15);
    level.entity_mut(player).set_shift_key_down(true);
    assert_eq!(interact(&mut level, player, donkey, None), InteractionResult::SUCCESS);
    let menu = level.player(player).container_menu.as_ref().expect("a horse menu");
    assert!(matches!(menu.kind, MenuKind::HorseInventory { inventory_columns: 5, .. }));
    assert_eq!(menu.slots.len(), 2 + 15 + 36);
}

#[test]
fn chested_donkeys_save_and_drop_their_chest() {
    let (mut level, player, donkey) = setup(Donkey::create());
    level.tame_with_name(donkey, player);
    interact(&mut level, player, donkey, Some(ItemStack::new(items::CHEST, 1)));
    level.mob_mut(donkey).unwrap().abstract_horse_mut().unwrap().inventory.items[7] = ItemStack::new(items::DIAMOND, 3);
    let nbt = level.save_entity(donkey).expect("a saved donkey");
    let LevelEntity::Mob(loaded) = level.load_entity(&nbt).expect("a loaded donkey") else { unreachable!() };
    assert!(abstract_chested_horse::has_chest(&loaded.entity));
    assert!(loaded.abstract_horse().unwrap().inventory.items[7].is(items::DIAMOND));
    assert_eq!(loaded.abstract_horse().unwrap().owner, level.mob(donkey).unwrap().abstract_horse().unwrap().owner);
    level.kill(donkey);
    let mut dropped = items_on_ground(&level);
    dropped.sort_by_key(|(item, _)| item.0);
    assert!(dropped.contains(&(items::DIAMOND, 3)));
    assert!(dropped.contains(&(items::CHEST, 1)));
}

#[test]
fn horses_and_donkeys_breed_mules_with_mixed_attributes() {
    let (mut level, player, horse) = setup(Horse::create());
    let mut mob = Donkey::create();
    mob.entity.set_pos(Vec3::new(2.5, -60.0, 2.5));
    let donkey = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    for parent in [horse, donkey] {
        level.tame_with_name(parent, player);
        interact(&mut level, player, parent, Some(ItemStack::new(items::GOLDEN_CARROT, 1)));
        assert!(level.is_in_love(parent));
    }
    for _ in 0..200 {
        level.tick();
    }
    let mule = level.entities.values().find_map(|e| match e {
        LevelEntity::Mob(mob) if matches!(mob.kind, MobKind::Mule(_)) => Some(mob),
        _ => None,
    });
    let mule = mule.expect("a mule");
    assert!(mule.is_baby());
    let health = mule.living.attributes.base_value(Attribute::MaxHealth);
    assert!((15.0..=30.0).contains(&health), "{health}");
    assert!(mule.item_by_slot(EquipmentSlot::Saddle).is_none());
}

#[test]
fn skeleton_and_zombie_horses_have_vanillas_attributes() {
    use crate::world::entity::attributes::Attribute;
    let skeleton_horse = super::skeleton_horse::SkeletonHorse::create();
    assert_eq!(skeleton_horse.living.attributes.value(Attribute::MaxHealth), 15.0);
    assert_eq!(skeleton_horse.living.attributes.value(Attribute::MovementSpeed), 0.2f32 as f64);
    let zombie_horse = super::zombie_horse::ZombieHorse::create();
    assert_eq!(zombie_horse.living.attributes.value(Attribute::MaxHealth), 25.0);
    let priorities: Vec<i32> = zombie_horse.ai.goal_selector.get_available_goals().iter().map(|wrapped| wrapped.get_priority()).collect();
    assert_eq!(priorities, vec![1, 2, 4, 6, 7, 8, 9, 0, 3]);
    let priorities: Vec<i32> = skeleton_horse.ai.goal_selector.get_available_goals().iter().map(|wrapped| wrapped.get_priority()).collect();
    assert_eq!(priorities, vec![1, 2, 4, 6, 7, 8, 9]);
}

#[test]
fn skeleton_trap_horses_keep_their_trap_goal_until_triggered() {
    use wafer_protocol::nbt::Nbt;
    let mut horse = super::skeleton_horse::SkeletonHorse::create();
    horse.load(&Nbt::Compound(vec![("SkeletonTrap".into(), Nbt::Byte(1)), ("SkeletonTrapTime".into(), Nbt::Int(5))]));
    assert!(super::skeleton_horse::SkeletonHorse::is_trap(&horse));
    assert_eq!(horse.ai.goal_selector.get_available_goals().last().unwrap().get_priority(), 1);
    super::skeleton_horse::SkeletonHorse::set_trap(&mut horse, false);
    assert_eq!(horse.ai.goal_selector.get_available_goals().len(), 7);
}
