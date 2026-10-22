use wafer_core::Vec3;
use wafer_data::{EquipmentSlot, items};

use super::sulfur_cube::SulfurCube;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::Mob;
use crate::world::entity::sulfur_cube_archetype::SulfurCubeArchetype;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::{InteractionHand, InteractionResult};

fn add(level: &mut ServerLevel, mut mob: Mob, pos: Vec3) -> EntityId {
    mob.entity.snap_to(pos.x, pos.y, pos.z, 0.0, 0.0);
    let id = mob.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(mob)));
    id
}

#[test]
fn vanilla_archetypes_load_in_registry_order() {
    let archetypes = SulfurCubeArchetype::registered();
    assert_eq!(archetypes.len(), 12);
    assert!(archetypes[1].explosion.is_some_and(|explosion| explosion.power == 3 && explosion.fuse == 120));
    assert!(archetypes[5].contact_damage.is_some());
    assert!(archetypes.iter().any(|archetype| archetype.matches(&ItemStack::new(items::DIRT, 1))));
}

#[test]
fn swallowed_blocks_change_attributes_and_shearing_ejects_them() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut cube = SulfurCube::create();
    cube.set_size(2, true);
    let cube = add(&mut level, cube, Vec3::new(0.5, -60.0, 2.5));
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::DIRT, 2)));
    assert_eq!(level.sulfur_cube_mob_interact(cube, player, InteractionHand::MainHand), InteractionResult::SUCCESS_SERVER);
    assert_eq!(level.item_by_slot(cube, EquipmentSlot::Body), Some(ItemStack::new(items::DIRT, 1)));
    assert_eq!(level.player(player).item_in_hand(InteractionHand::MainHand).map(|stack| stack.count), Some(1));
    level.tick();
    assert_eq!(level.attribute_value(cube, Attribute::KnockbackResistance), -1.0);
    assert!(level.attribute_value(cube, Attribute::Bounciness) > 0.0);
    level.player_mut(player).set_item_in_hand(InteractionHand::MainHand, Some(ItemStack::new(items::SHEARS, 1)));
    assert_eq!(level.sulfur_cube_mob_interact(cube, player, InteractionHand::MainHand), InteractionResult::SUCCESS);
    assert_eq!(level.item_by_slot(cube, EquipmentSlot::Body), None);
    level.tick();
    assert_eq!(level.attribute_value(cube, Attribute::Bounciness), 0.0);
}

#[test]
fn adults_split_into_two_babies() {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    let mut cube = SulfurCube::create();
    cube.set_size(2, true);
    let cube = add(&mut level, cube, Vec3::new(0.5, -60.0, 4.5));
    level.kill(cube);
    for _ in 0..25 {
        level.tick();
    }
    let babies: Vec<EntityId> = level.entities.iter().filter(|(_, e)| matches!(e, LevelEntity::Mob(mob) if mob.sulfur_cube().is_some() && !mob.entity.is_removed())).map(|(id, _)| *id).collect();
    assert_eq!(babies.len(), 2);
    for baby in babies {
        assert!(level.is_baby(baby));
        assert_eq!(level.mob(baby).expect("a sulfur cube").get_size(), 1);
        assert_eq!(level.attribute_value(baby, Attribute::MaxHealth), 4.0);
    }
}
