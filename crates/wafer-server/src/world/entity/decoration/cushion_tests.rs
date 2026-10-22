use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::{blocks, items};

use super::cushion::Cushion;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::dye_color::DyeColor;
use crate::world::level::block::BlockHitResult;
use crate::world::{InteractionHand, InteractionResult};

fn add_cushion(level: &mut ServerLevel, pos: Vec3, color: DyeColor) -> EntityId {
    let mut cushion = Cushion::of_type();
    cushion.set_color(color);
    cushion.entity.set_pos(pos);
    let id = cushion.entity.id;
    level.add_fresh_entity(LevelEntity::Cushion(cushion));
    id
}

fn top_of(pos: BlockPos) -> BlockHitResult {
    BlockHitResult { location: Vec3::new(pos.x as f64 + 0.5, pos.y as f64 + 1.0, pos.z as f64 + 0.5), direction: Direction::Up, block_pos: pos, inside: false, world_border_hit: false }
}

#[test]
fn cushions_are_placed_on_top_faces_only() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 3.5), 180.0, 30.0);
    let stack = ItemStack::new(items::RED_CUSHION, 2);
    let side = BlockHitResult { direction: Direction::North, ..top_of(BlockPos::new(0, -61, 0)) };
    assert_eq!(crate::world::item::cushion_item::use_on(&mut level, player, InteractionHand::MainHand, side, stack.clone()), InteractionResult::Fail);
    assert_eq!(crate::world::item::cushion_item::use_on(&mut level, player, InteractionHand::MainHand, top_of(BlockPos::new(0, -61, 0)), stack.clone()), InteractionResult::SUCCESS);
    let cushion = level.entities.values().find_map(|e| match e {
        LevelEntity::Cushion(cushion) => Some(cushion),
        _ => None,
    });
    let cushion = cushion.expect("a placed cushion");
    assert_eq!(cushion.get_color(), DyeColor::by_name("red").unwrap());
    assert_eq!(cushion.entity.position(), Vec3::new(0.5, -60.0, 0.5));
    assert_eq!(crate::world::item::cushion_item::use_on(&mut level, player, InteractionHand::MainHand, top_of(BlockPos::new(0, -61, 0)), stack), InteractionResult::Fail);
}

#[test]
fn players_sit_on_cushions() {
    let mut level = testing::level();
    let player = testing::player(&mut level, Vec3::new(0.5, -60.0, 2.5), 180.0, 0.0);
    let id = add_cushion(&mut level, Vec3::new(0.5, -60.0, 0.5), DyeColor::VALUES[0]);
    assert_eq!(level.interact(id, player, InteractionHand::MainHand, Vec3::ZERO), InteractionResult::SUCCESS_SERVER);
    assert_eq!(level.vehicle(player), Some(id));
    level.stop_riding(player);
    assert!(level.vehicle(player).is_none());
    assert!(!level.entity(id).is_removed());
}

#[test]
fn unsupported_cushions_drop_after_the_check_interval() {
    let mut level = testing::level();
    let _player = testing::player(&mut level, Vec3::new(0.5, -60.0, 8.5), 180.0, 0.0);
    let id = add_cushion(&mut level, Vec3::new(0.5, -60.0, 0.5), DyeColor::by_name("blue").unwrap());
    level.set_block(BlockPos::new(0, -61, 0), blocks::AIR.default_state(), 3);
    for _ in 0..100 {
        level.tick();
    }
    assert!(!level.entity(id).is_removed());
    level.tick();
    assert!(level.entity_ref(id).is_none_or(|e| e.is_removed()));
    assert!(level.entities.values().any(|e| matches!(e, LevelEntity::Item(item) if item.item.is(items::BLUE_CUSHION))));
}
