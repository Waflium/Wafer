use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, Vec3};
use wafer_data::{EquipmentSlot, blocks, items};

use super::sulfur_cube::SulfurCube;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::attributes::Attribute;
use crate::world::entity::mob::Mob;
use crate::world::entity::sulfur_cube_archetype::SulfurCubeArchetype;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::level::update_flags::UPDATE_ALL;
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
fn player_push_scales_with_the_client_reported_movement() {
    let push = |known_movement: Vec3| {
        let mut level = testing::level();
        let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
        let mut cube = SulfurCube::create();
        cube.set_size(2, true);
        let cube = add(&mut level, cube, Vec3::new(0.5, -60.0, 1.5));
        level.set_item_slot(cube, EquipmentSlot::Body, ItemStack::new(items::DIRT, 1));
        level.player_mut(player).known_movement = known_movement;
        level.entity_mut(player).last_known_speed = Vec3::new(0.0, 0.0, 0.5);
        level.entity_mut(cube).delta_movement = Vec3::ZERO;
        level.sulfur_cube_player_touch(cube, player);
        level.entity(cube).delta_movement.z
    };
    assert_eq!(push(Vec3::ZERO), 0.0);
    assert!((push(Vec3::new(0.0, 0.0, 0.25)) - 0.15).abs() < 1.0e-6);
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

fn growing_cube_in_water(seed: i64) -> (ServerLevel, EntityId) {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    for x in 1..=6 {
        for y in -60..=-55 {
            for z in 1..=6 {
                level.set_block(BlockPos::new(x, y, z), blocks::WATER.default_state(), UPDATE_ALL);
            }
        }
    }
    let mut cube = SulfurCube::create();
    cube.set_no_ai(true);
    cube.set_size(1, true);
    let cube = add(&mut level, cube, Vec3::new(3.5, -58.0, 3.5));
    level.tick();
    assert!(level.entity(cube).is_in_water());
    level.entity_mut(cube).random = LegacyRandomSource::new(seed);
    level.set_age(cube, 0);
    assert_eq!(level.mob(cube).expect("a sulfur cube").get_size(), 2);
    (level, cube)
}

#[test]
fn growing_up_in_water_rolls_for_a_splash() {
    let roll = |seed: i64| LegacyRandomSource::new(seed).next_int_bounded(20);
    let skipped = (0..).find(|&seed| roll(seed) != 0).unwrap();
    let splashed = (0..).find(|&seed| roll(seed) == 0).unwrap();
    let (mut level, cube) = growing_cube_in_water(skipped);
    let mut reference = LegacyRandomSource::new(skipped);
    reference.next_int_bounded(20);
    assert_eq!(level.entity_mut(cube).random.next_long(), reference.next_long());
    let (mut level, cube) = growing_cube_in_water(splashed);
    let mut reference = LegacyRandomSource::new(splashed);
    reference.next_int_bounded(20);
    assert_ne!(level.entity_mut(cube).random.next_long(), reference.next_long());
}

#[test]
fn loading_an_adult_age_grows_the_cube() {
    let mut cube = SulfurCube::create();
    cube.set_size(1, true);
    cube.set_age(-24000);
    cube.set_age(0);
    assert_eq!(cube.get_size(), 2);
}
