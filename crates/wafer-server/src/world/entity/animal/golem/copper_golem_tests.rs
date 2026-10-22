use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::properties::{CHEST_TYPE, ChestType, HORIZONTAL_FACING};
use wafer_data::{EquipmentSlot, blocks, items};

use super::copper_golem::{self, CopperGolemState};
use crate::server::level::{ServerLevel, testing};
use crate::world::container::ContainerRef;
use crate::world::entity::mob::MobKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::BlockGetter;
use crate::world::level::block::BlockHitResult;
use crate::world::level::block::weathering_copper::WeatherState;
use crate::world::level::update_flags::UPDATE_ALL;
use crate::world::{InteractionHand, InteractionResult};

fn copper_golems(level: &ServerLevel) -> Vec<EntityId> {
    level.entities.iter().filter_map(|(id, e)| matches!(e, LevelEntity::Mob(mob) if matches!(mob.kind, MobKind::CopperGolem(_))).then_some(*id)).collect()
}

fn add_copper_golem(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let mut entity = ServerLevel::create_entity("minecraft:copper_golem").expect("a copper golem");
    entity.entity_mut().set_pos(pos);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

#[test]
fn placing_a_pumpkin_on_copper_builds_a_copper_golem_on_a_copper_chest() {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.set_block(BlockPos::new(3, -60, 3), blocks::EXPOSED_COPPER.default_state(), UPDATE_ALL);
    level.set_block(BlockPos::new(3, -59, 3), blocks::CARVED_PUMPKIN.default_state().set(HORIZONTAL_FACING, Direction::East), UPDATE_ALL);
    let golems = copper_golems(&level);
    assert_eq!(golems.len(), 1);
    let e = level.entity(golems[0]);
    assert_eq!((e.x(), e.y(), e.z()), (3.5, -58.95, 3.5));
    assert_eq!(copper_golem::get_weather_state(e), WeatherState::Exposed);
    let chest = level.block_state(BlockPos::new(3, -60, 3));
    assert!(chest.is_block(blocks::EXPOSED_COPPER_CHEST));
    assert_eq!(chest.get(HORIZONTAL_FACING), Direction::East);
    assert_eq!(chest.get(CHEST_TYPE), ChestType::Single);
    assert!(level.block_state(BlockPos::new(3, -59, 3)).is_air());
}

#[test]
fn waxed_copper_builds_an_unwaxed_golem_on_a_waxed_chest() {
    let mut level = testing::level();
    level.set_block(BlockPos::new(3, -60, 3), blocks::WAXED_WEATHERED_COPPER.default_state(), UPDATE_ALL);
    level.set_block(BlockPos::new(3, -59, 3), blocks::CARVED_PUMPKIN.default_state(), UPDATE_ALL);
    let golems = copper_golems(&level);
    assert_eq!(golems.len(), 1);
    assert_eq!(copper_golem::get_weather_state(level.entity(golems[0])), WeatherState::Weathered);
    assert!(level.block_state(BlockPos::new(3, -60, 3)).is_block(blocks::WAXED_WEATHERED_COPPER_CHEST));
}

#[test]
fn copper_golems_carry_items_from_copper_chests_to_chests() {
    let mut level = testing::level();
    let source = BlockPos::new(2, -60, 0);
    let destination = BlockPos::new(-2, -60, 0);
    level.set_block(source, blocks::COPPER_CHEST.default_state(), UPDATE_ALL);
    level.set_block(destination, blocks::CHEST.default_state(), UPDATE_ALL);
    let source_container = ContainerRef::block(&level, source);
    level.set_item(source_container, 0, ItemStack::new(items::DIRT, 40));
    testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let golem = add_copper_golem(&mut level, Vec3::new(0.5, -60.0, 2.5));
    let mut saw_getting_item = false;
    for _ in 0..1200 {
        level.tick();
        saw_getting_item |= copper_golem::get_state(level.entity(golem)) == CopperGolemState::GettingItem;
    }
    assert!(saw_getting_item);
    let destination_container = ContainerRef::block(&level, destination);
    let moved = (0..27).map(|slot| level.get_item(destination_container, slot).count).sum::<i32>();
    let left = (0..27).map(|slot| level.get_item(source_container, slot).count).sum::<i32>();
    let held = level.item_by_slot(golem, EquipmentSlot::Mainhand).map_or(0, |stack| stack.count);
    assert!(moved >= 16, "moved {moved}, left {left}, held {held}");
    assert_eq!(moved + left + held, 40);
}

#[test]
fn copper_golems_save_their_weathering() {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let golem = add_copper_golem(&mut level, Vec3::new(0.5, -60.0, 0.5));
    copper_golem::set_weather_state(level.entity_mut(golem), WeatherState::Weathered);
    level.tick();
    let saved = level.save_entity(golem).expect("a saved copper golem");
    assert_eq!(saved.get("weather_state").and_then(|v| v.as_str()), Some("weathered"));
    assert!(saved.get("next_weather_age").and_then(|v| v.as_i64()).is_some_and(|age| age >= 504000));
    let loaded = level.load_entity(&saved).expect("a loaded copper golem");
    assert_eq!(copper_golem::get_weather_state(loaded.entity()), WeatherState::Weathered);
}

#[test]
fn honeycomb_waxes_and_axes_scrape_copper_golems() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 0.0);
    let golem = add_copper_golem(&mut level, Vec3::new(0.5, -60.0, 0.5));
    copper_golem::set_weather_state(level.entity_mut(golem), WeatherState::Oxidized);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::IRON_AXE, 1)));
    assert!(matches!(level.copper_golem_mob_interact(golem, player, InteractionHand::MainHand), InteractionResult::Success { .. }));
    assert_eq!(copper_golem::get_weather_state(level.entity(golem)), WeatherState::Weathered);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::HONEYCOMB, 1)));
    assert!(matches!(level.copper_golem_mob_interact(golem, player, InteractionHand::MainHand), InteractionResult::Success { .. }));
    assert!(level.player(player).item_in_hand(InteractionHand::MainHand).is_none_or(|stack| stack.is_empty()));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::IRON_AXE, 1)));
    assert!(matches!(level.copper_golem_mob_interact(golem, player, InteractionHand::MainHand), InteractionResult::Success { .. }));
    assert_eq!(copper_golem::get_weather_state(level.entity(golem)), WeatherState::Weathered);
    assert!(matches!(level.copper_golem_mob_interact(golem, player, InteractionHand::MainHand), InteractionResult::Success { .. }));
    assert_eq!(copper_golem::get_weather_state(level.entity(golem)), WeatherState::Exposed);
}

#[test]
fn axes_wake_unaffected_copper_golem_statues() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 0.0);
    let pos = BlockPos::new(0, -60, 0);
    level.set_block(pos, blocks::COPPER_GOLEM_STATUE.default_state().set(HORIZONTAL_FACING, Direction::South), UPDATE_ALL);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::IRON_AXE, 1)));
    let hit = BlockHitResult { location: Vec3::new(0.5, -59.0, 0.5), direction: Direction::Up, block_pos: pos, inside: false, world_border_hit: false };
    assert_eq!(level.use_item_on(player, InteractionHand::MainHand, hit), InteractionResult::SUCCESS);
    assert!(level.block_state(pos).is_air());
    let golems = copper_golems(&level);
    assert_eq!(golems.len(), 1);
    let e = level.entity(golems[0]);
    assert_eq!((e.x(), e.y(), e.z(), e.y_rot), (0.5, -60.0, 0.5, 0.0));
}
