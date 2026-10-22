use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, items};

use super::allay;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::item::ItemEntity;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

fn add_allay(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let mut entity = ServerLevel::create_entity("minecraft:allay").expect("an allay");
    entity.entity_mut().set_pos(pos);
    let id = entity.entity().id;
    level.add_fresh_entity(entity);
    id
}

fn items_of(level: &ServerLevel, item: wafer_data::Item) -> i32 {
    level.entities.values().filter_map(|e| match e {
        LevelEntity::Item(entity) if entity.item.is(item) => Some(entity.item.count),
        _ => None,
    }).sum()
}

#[test]
fn allays_take_items_and_like_the_giver() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 0.0);
    let allay = add_allay(&mut level, Vec3::new(0.5, -59.0, 0.5));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::DIAMOND, 3)));
    assert_eq!(level.allay_mob_interact(allay, player, InteractionHand::MainHand), InteractionResult::SUCCESS);
    assert_eq!(level.item_by_slot(allay, EquipmentSlot::Mainhand).map(|stack| stack.count), Some(1));
    assert_eq!(level.player(player).item_in_hand(InteractionHand::MainHand).map(|stack| stack.count), Some(2));
    let uuid = level.entity(player).uuid;
    assert_eq!(level.get_brain(allay).get_memory(memory_module_type::LIKED_PLAYER).copied(), Some(uuid));
    assert!(level.mob(allay).expect("an allay").can_pick_up_loot());
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, None);
    assert_eq!(level.allay_mob_interact(allay, player, InteractionHand::MainHand), InteractionResult::SUCCESS);
    assert!(level.item_by_slot(allay, EquipmentSlot::Mainhand).is_none());
    assert!(level.get_brain(allay).get_memory(memory_module_type::LIKED_PLAYER).is_none());
}

#[test]
fn allays_bring_matching_items_to_their_player() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let allay = add_allay(&mut level, Vec3::new(0.5, -59.0, 1.5));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::DIAMOND, 1)));
    level.allay_mob_interact(allay, player, InteractionHand::MainHand);
    let mut item = ItemEntity::new(Vec3::new(8.5, -60.0, 8.5), ItemStack::new(items::DIAMOND, 3));
    item.pickup_delay = 0;
    level.add_fresh_entity(LevelEntity::Item(item));
    let mut picked_up = false;
    for _ in 0..600 {
        level.tick();
        picked_up |= !allay::allay(level.mob(allay).expect("an allay")).inventory.is_empty();
    }
    assert!(picked_up);
    assert!(allay::allay(level.mob(allay).expect("an allay")).inventory.is_empty());
    assert_eq!(items_of(&level, items::DIAMOND) + level.player(player).inventory.slots.iter().flatten().filter(|stack| stack.is(items::DIAMOND)).map(|stack| stack.count).sum::<i32>(), 3);
}

#[test]
fn allays_save_their_inventory_and_duplication_cooldown() {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let allay = add_allay(&mut level, Vec3::new(0.5, -59.0, 0.5));
    allay::allay_mut(level.mob_mut(allay).expect("an allay")).inventory.set_item(0, ItemStack::new(items::DIAMOND, 5));
    let saved = level.save_entity(allay).expect("a saved allay");
    assert_eq!(saved.get("DuplicationCooldown").and_then(|v| v.as_i64()), Some(0));
    assert!(saved.get("listener").is_some());
    let loaded = level.load_entity(&saved).expect("a loaded allay");
    let LevelEntity::Mob(mob) = loaded else { panic!("a mob") };
    assert_eq!(allay::allay(&mob).inventory.get_item(0).count, 5);
}
