use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, items};

use super::piglin::{self, Piglin};
use super::piglin_ai;
use super::piglin_brute::PiglinBrute;
use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::entity::ai::memory::memory_module_type;
use crate::world::entity::mob::{Mob, MobKind};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;

fn add(level: &mut ServerLevel, mut mob: Mob, x: f64) -> EntityId {
    mob.entity.snap_to(x, -60.0, 4.5, 0.0, 0.0);
    mob.persistence_required = true;
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

#[test]
fn piglins_turn_into_zombified_piglins_and_drop_their_inventory() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = Piglin::create();
    if let MobKind::Piglin(piglin) = &mut mob.kind {
        piglin.abstract_piglin.time_in_overworld = 300;
        piglin.inventory.add_item(&ItemStack::new(items::GOLD_NUGGET, 3));
    }
    let id = add(&mut level, mob, 2.5);
    for _ in 0..3 {
        level.tick();
    }
    assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()), "the piglin is still there");
    assert!(level.entities.values().any(|e| matches!(e, LevelEntity::Mob(m) if matches!(m.kind, MobKind::ZombifiedPiglin(_)))), "no zombified piglin");
    assert!(level.entities.values().any(|e| matches!(e, LevelEntity::Item(item) if item.item.is(items::GOLD_NUGGET) && item.item.count == 3)), "the inventory was not dropped");
}

#[test]
fn immune_piglins_stay_piglins() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut mob = PiglinBrute::create();
    super::abstract_piglin::set_immune_to_zombification(&mut mob.entity, true);
    let id = add(&mut level, mob, 2.5);
    for _ in 0..310 {
        level.tick();
    }
    assert!(level.entity_ref(id).is_some_and(|e| !e.is_removed()));
}

#[test]
fn adult_piglins_admire_gold_handed_over_by_players() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::GOLD_INGOT, 4)));
    let piglin = add(&mut level, Piglin::create(), 2.5);
    let mut baby = Piglin::create();
    piglin::set_baby(&mut baby, true);
    let baby = add(&mut level, baby, 4.5);
    assert!(!piglin_ai::mob_interact(&mut level, baby, player, InteractionHand::MainHand).consumes_action());
    assert!(piglin_ai::mob_interact(&mut level, piglin, player, InteractionHand::MainHand).consumes_action());
    assert!(level.item_by_slot_ref(piglin, EquipmentSlot::Offhand).is_some_and(|item| item.is(items::GOLD_INGOT) && item.count == 1));
    assert!(level.get_brain(piglin).has_memory_value(memory_module_type::ADMIRING_ITEM));
    assert_eq!(level.get_item_in_hand(player, InteractionHand::MainHand).map(|item| item.count), Some(3));
    assert!(!piglin_ai::can_admire(&level, piglin, &ItemStack::new(items::GOLD_INGOT, 1)));
}

#[test]
fn piglins_want_gold_but_not_repellents() {
    let mut level = testing::level();
    let piglin = add(&mut level, Piglin::create(), 2.5);
    let brute = add(&mut level, PiglinBrute::create(), 6.5);
    assert!(piglin_ai::wants_to_pickup(&level, piglin, &ItemStack::new(items::GOLD_NUGGET, 1)));
    assert!(piglin_ai::wants_to_pickup(&level, piglin, &ItemStack::new(items::GOLD_INGOT, 1)));
    assert!(!piglin_ai::wants_to_pickup(&level, piglin, &ItemStack::new(items::SOUL_TORCH, 1)));
    assert!(level.wants_to_pick_up(brute, &ItemStack::new(items::GOLDEN_AXE, 1)));
    assert!(!level.wants_to_pick_up(brute, &ItemStack::new(items::GOLD_INGOT, 1)));
}
