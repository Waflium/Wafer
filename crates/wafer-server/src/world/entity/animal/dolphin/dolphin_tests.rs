use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, items};

use super::dolphin::{self, Dolphin};
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::entity::{EntityId, ItemEntity, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::update_flags::UPDATE_ALL;

fn add_dolphin(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let mut mob = Dolphin::create();
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    mob.persistence_required = true;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

fn fill_pool(level: &mut ServerLevel) {
    for x in -10..=10 {
        for z in 5..=25 {
            for y in -63..=-61 {
                level.set_block(BlockPos::new(x, y, z), blocks::WATER.default_state(), UPDATE_ALL);
            }
        }
    }
}

#[test]
fn dolphins_dry_out_and_flop_on_land() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let id = add_dolphin(&mut level, Vec3::new(20.5, -60.0, 0.5));
    let mut flopped = false;
    for _ in 0..40 {
        level.tick();
        flopped |= level.entity(id).y() > -59.5;
    }
    assert_eq!(dolphin::get_moistness_level(level.entity(id)), 2400 - 40);
    assert!(flopped, "the dolphin never flopped");
}

#[test]
fn dolphins_eat_fish_and_give_up_without_treasure() {
    let mut level = testing::level();
    fill_pool(&mut level);
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 0.0, 0.0);
    let id = add_dolphin(&mut level, Vec3::new(0.5, -62.0, 6.5));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::COD, 2)));
    let position = level.entity(id).position();
    level.interact(id, player, InteractionHand::MainHand, position);
    assert!(dolphin::got_fish(level.entity(id)));
    assert_eq!(level.player(player).item_in_hand(InteractionHand::MainHand).map(|stack| stack.count), Some(1));
    for _ in 0..10 {
        level.tick();
    }
    assert!(!dolphin::got_fish(level.entity(id)), "the dolphin kept searching for treasure in a world without structures");
}

#[test]
fn dolphins_pick_up_and_throw_items_in_water() {
    let mut level = testing::level();
    fill_pool(&mut level);
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let id = add_dolphin(&mut level, Vec3::new(0.5, -62.0, 15.5));
    let mut item = ItemEntity::new(Vec3::new(1.5, -62.0, 15.5), ItemStack::new(items::COD, 1));
    item.pickup_delay = 0;
    level.add_fresh_entity(LevelEntity::Item(item));
    let mut held = false;
    for _ in 0..200 {
        level.tick();
        held |= level.item_by_slot_ref(id, EquipmentSlot::Mainhand).is_some();
    }
    assert!(held, "the dolphin never picked up the cod");
}
