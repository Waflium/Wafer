use wafer_core::{BlockPos, Vec3};
use wafer_data::{blocks, items};

use crate::server::level::{ServerLevel, testing};
use crate::world::InteractionHand;
use crate::world::entity::data::DataValue;
use crate::world::entity::monster::zombie::zombie::Zombie;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::{self, ItemStack};
use crate::world::level::update_flags::UPDATE_ALL;

fn fisher(level: &mut ServerLevel, y_rot: f32, x_rot: f32) -> EntityId {
    let player = testing::player(level, Vec3::new(0.5, -60.0, 0.5), y_rot, x_rot);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::FISHING_ROD, 1)));
    player
}

fn use_rod(level: &mut ServerLevel, player: EntityId) {
    let stack = level.player(player).item_in_hand(InteractionHand::MainHand).expect("a rod");
    item::use_item(level, player, InteractionHand::MainHand, stack);
}

fn rod_damage(level: &ServerLevel, player: EntityId) -> i32 {
    level.player(player).item_in_hand(InteractionHand::MainHand).expect("a rod").damage_value()
}

fn fill_pond(level: &mut ServerLevel) {
    for x in -6..=6 {
        for z in 2..=14 {
            for y in -63..=-61 {
                level.set_block(BlockPos::new(x, y, z), blocks::WATER.default_state(), UPDATE_ALL);
            }
        }
    }
}

#[test]
fn casting_and_reeling_in_a_hook() {
    let mut level = testing::level();
    let player = fisher(&mut level, 0.0, 20.0);
    use_rod(&mut level, player);
    let hook = level.player(player).fishing.expect("a hook");
    assert!(level.is_fishing_hook(hook));
    assert_eq!(level.fishing_hook_get_player_owner(hook), Some(player));
    let e = level.entity(hook);
    assert_eq!(e.y(), level.entity(player).eye_y());
    assert!((e.z() - 0.8).abs() < 1e-6 && (e.x() - 0.5).abs() < 1e-6, "{:?}", e.position());
    for _ in 0..5 {
        level.tick();
    }
    use_rod(&mut level, player);
    assert_eq!(level.player(player).fishing, None);
    assert!(level.entity_ref(hook).is_none_or(|e| e.is_removed()));
}

#[test]
fn hooks_are_removed_when_the_rod_is_put_away() {
    let mut level = testing::level();
    let player = fisher(&mut level, 0.0, 20.0);
    use_rod(&mut level, player);
    let hook = level.player(player).fishing.expect("a hook");
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, None);
    level.tick();
    assert!(level.entity_ref(hook).is_none_or(|e| e.is_removed()));
    assert_eq!(level.player(player).fishing, None);
}

#[test]
fn reeling_in_a_bite_drops_fishing_loot() {
    let mut level = testing::level();
    fill_pond(&mut level);
    let player = fisher(&mut level, 0.0, 20.0);
    use_rod(&mut level, player);
    let hook = level.player(player).fishing.expect("a hook");
    let mut bit = false;
    for _ in 0..2000 {
        level.tick();
        if matches!(level.entity(hook).data.get(9), DataValue::Boolean(true)) {
            bit = true;
            break;
        }
    }
    assert!(bit, "no fish bit");
    assert_eq!(level.fishing_hook_open_water(hook), Some(true));
    use_rod(&mut level, player);
    assert!(level.entities.values().any(|e| matches!(e, LevelEntity::Item(_))), "no loot");
    assert!(level.entities.values().any(|e| matches!(e, LevelEntity::ExperienceOrb(_))), "no experience");
    assert_eq!(rod_damage(&level, player), 1);
}

#[test]
fn hooked_entities_are_pulled_towards_the_fisher() {
    let mut level = testing::level();
    let player = fisher(&mut level, 0.0, 0.0);
    let mut zombie = Zombie::create();
    zombie.entity.snap_to(0.5, -60.0, 4.5, 0.0, 0.0);
    zombie.set_no_ai(true);
    zombie.persistence_required = true;
    let zombie_id = zombie.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(zombie)));
    use_rod(&mut level, player);
    let hook = level.player(player).fishing.expect("a hook");
    let mut hooked = false;
    for _ in 0..20 {
        level.tick();
        if matches!(level.entity(hook).data.get(8), DataValue::Int(id) if *id == zombie_id + 1) {
            hooked = true;
            break;
        }
    }
    assert!(hooked, "the zombie was not hooked");
    let before = level.entity(zombie_id).delta_movement;
    use_rod(&mut level, player);
    let after = level.entity(zombie_id).delta_movement;
    assert!(after.z < before.z, "{before:?} -> {after:?}");
    assert_eq!(rod_damage(&level, player), 5);
    assert_eq!(level.player(player).fishing, None);
}
