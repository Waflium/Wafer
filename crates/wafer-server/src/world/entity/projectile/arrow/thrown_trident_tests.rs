use wafer_core::Vec3;
use wafer_data::items;
use wafer_protocol::nbt::Nbt;

use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::entity::projectile::projectile::ProjectileKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment_helper;

fn trident(enchantments: &[(&str, i32)]) -> ItemStack {
    let mut stack = ItemStack::new(items::TRIDENT, 1);
    for &(enchantment, level) in enchantments {
        enchantment_helper::enchant(&mut stack, enchantment, level);
    }
    stack
}

fn tridents(level: &ServerLevel) -> Vec<EntityId> {
    level
        .entities
        .iter()
        .filter(|(_, e)| matches!(e, LevelEntity::Projectile(p) if matches!(p.kind, ProjectileKind::ThrownTrident(_))))
        .map(|(&id, _)| id)
        .collect()
}

fn throw(level: &mut ServerLevel, player: EntityId, stack: ItemStack) {
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(stack.clone()));
    crate::world::item::use_item(level, player, InteractionHand::MainHand, stack);
    for _ in 0..12 {
        level.tick();
        level.server_player_do_tick(player);
    }
    level.release_using_item(player);
}

#[test]
fn thrown_tridents_fly_and_stick() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, -10.0);
    throw(&mut level, player, trident(&[]));
    assert!(level.player(player).item_in_hand(InteractionHand::MainHand).is_none());
    let [trident] = tridents(&level)[..] else { panic!("no trident") };
    for _ in 0..80 {
        level.tick();
    }
    assert!(level.projectile(trident).is_in_ground());
    let mut out = Vec::new();
    level.save_without_id(trident, &mut out);
    let get = |key: &str| out.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone());
    assert_eq!(get("DealtDamage"), Some(Nbt::Byte(1)));
    assert_eq!(get("pickup"), Some(Nbt::Byte(1)));
    assert_eq!(get("SoundEvent"), Some(Nbt::String("minecraft:item.trident.hit_ground".into())));
}

#[test]
fn loyal_tridents_return_to_their_owner() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, -10.0);
    throw(&mut level, player, trident(&[("minecraft:loyalty", 3)]));
    let [trident] = tridents(&level)[..] else { panic!("no trident") };
    let mut returned = false;
    for _ in 0..200 {
        level.tick();
        level.server_player_do_tick(player);
        if !level.entities.contains_key(&trident) {
            returned = true;
            break;
        }
    }
    assert!(returned, "the loyal trident never came back");
    assert!(level.player(player).inventory.slots.iter().flatten().any(|s| s.is(items::TRIDENT)));
}
