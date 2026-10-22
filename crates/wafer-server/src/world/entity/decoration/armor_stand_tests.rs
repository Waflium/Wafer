use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, damage_types, items};

use super::armor_stand::ArmorStand;
use crate::server::level::{ServerLevel, testing};
use crate::world::damagesource::DamageSource;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

fn add_stand(level: &mut ServerLevel, pos: Vec3) -> EntityId {
    let stand = ArmorStand::at(pos.x, pos.y, pos.z);
    let id = stand.entity.id;
    level.add_fresh_entity(LevelEntity::ArmorStand(Box::new(stand)));
    id
}

#[test]
fn players_put_armor_on_stands_and_take_it_back() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 0.0);
    let id = add_stand(&mut level, Vec3::new(0.5, -60.0, 0.5));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::IRON_HELMET, 1)));
    let result = level.interact(id, player, InteractionHand::MainHand, Vec3::new(0.0, 1.0, 0.0));
    assert_eq!(result, InteractionResult::SUCCESS_SERVER);
    assert!(level.item_by_slot(id, EquipmentSlot::Head).is_some_and(|stack| stack.is(items::IRON_HELMET)));
    assert!(level.player(player).item_in_hand(InteractionHand::MainHand).is_none_or(|stack| stack.is_empty()));
    let result = level.interact(id, player, InteractionHand::MainHand, Vec3::new(0.0, 1.8, 0.0));
    assert_eq!(result, InteractionResult::SUCCESS_SERVER);
    assert!(level.item_by_slot(id, EquipmentSlot::Head).is_none());
    assert!(level.player(player).item_in_hand(InteractionHand::MainHand).is_some_and(|stack| stack.is(items::IRON_HELMET)));
}

#[test]
fn stands_wobble_then_break_and_drop_themselves() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 0.0);
    let id = add_stand(&mut level, Vec3::new(0.5, -60.0, 0.5));
    level.tick();
    level.game_time = 100;
    assert!(level.hurt_server(id, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 1.0));
    assert!(!level.entity(id).is_removed());
    assert!(level.hurt_server(id, DamageSource::entity(damage_types::PLAYER_ATTACK, player), 1.0));
    assert!(level.entity(id).is_removed());
    assert!(level.entities.values().any(|e| matches!(e, LevelEntity::Item(item) if item.item.is(items::ARMOR_STAND))));
}

#[test]
fn stands_fall_and_save_their_flags() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add_stand(&mut level, Vec3::new(0.5, -55.0, 0.5));
    for _ in 0..40 {
        level.tick();
    }
    assert_eq!(level.entity(id).y(), -60.0);
    let mut out = Vec::new();
    level.save_without_id(id, &mut out);
    let keys: Vec<&str> = out.iter().map(|(k, _)| k.as_str()).collect();
    for key in ["Invisible", "Small", "ShowArms", "DisabledSlots", "NoBasePlate", "Pose", "Health", "Brain"] {
        assert!(keys.contains(&key), "missing {key}");
    }
    assert!(!keys.contains(&"Marker"));
    assert!(!keys.contains(&"PersistenceRequired"));
}
