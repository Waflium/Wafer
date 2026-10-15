use wafer_core::{BlockPos, Vec3};
use wafer_data::blocks;

use super::iron_golem;
use crate::server::level::{ServerLevel, testing};
use crate::world::entity::mob::MobKind;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::level::BlockGetter;
use crate::world::level::block::carved_pumpkin_block;
use crate::world::level::update_flags::UPDATE_ALL;

fn build_level() -> ServerLevel {
    let mut level = testing::level();
    testing::player(&mut level, Vec3::new(0.5, -60.0, 0.5), 0.0, 0.0);
    level
}

fn golems(level: &ServerLevel) -> Vec<(EntityId, &'static str)> {
    level
        .entities
        .iter()
        .filter_map(|(id, e)| match e {
            LevelEntity::Mob(mob) if matches!(mob.kind, MobKind::IronGolem(_)) => Some((*id, "iron_golem")),
            LevelEntity::Mob(mob) if matches!(mob.kind, MobKind::SnowGolem(_)) => Some((*id, "snow_golem")),
            _ => None,
        })
        .collect()
}

fn place_iron_body(level: &mut ServerLevel, center: BlockPos) {
    for pos in [center.below(), center.offset(-1, 0, 0), center, center.offset(1, 0, 0)] {
        level.set_block(pos, blocks::IRON_BLOCK.default_state(), UPDATE_ALL);
    }
}

#[test]
fn placing_a_pumpkin_on_snow_builds_a_snow_golem() {
    let mut level = build_level();
    level.set_block(BlockPos::new(3, -60, 3), blocks::SNOW_BLOCK.default_state(), UPDATE_ALL);
    level.set_block(BlockPos::new(3, -59, 3), blocks::SNOW_BLOCK.default_state(), UPDATE_ALL);
    assert!(golems(&level).is_empty());
    assert!(carved_pumpkin_block::can_spawn_golem(&mut level, BlockPos::new(3, -58, 3)));
    level.set_block(BlockPos::new(3, -58, 3), blocks::CARVED_PUMPKIN.default_state(), UPDATE_ALL);
    let golems = golems(&level);
    assert_eq!(golems.len(), 1);
    assert_eq!(golems[0].1, "snow_golem");
    let e = level.entity(golems[0].0);
    assert_eq!((e.x(), e.y(), e.z(), e.y_rot), (3.5, -59.95, 3.5, 0.0));
    for y in -60..=-58 {
        assert!(level.block_state(BlockPos::new(3, y, 3)).is_air());
    }
}

#[test]
fn placing_a_jack_o_lantern_on_iron_builds_a_player_created_iron_golem() {
    let mut level = build_level();
    for pos in [BlockPos::new(5, -60, 5), BlockPos::new(5, -59, 4), BlockPos::new(5, -59, 5), BlockPos::new(5, -59, 6)] {
        level.set_block(pos, blocks::IRON_BLOCK.default_state(), UPDATE_ALL);
    }
    level.set_block(BlockPos::new(5, -58, 5), blocks::JACK_O_LANTERN.default_state(), UPDATE_ALL);
    let golems = golems(&level);
    assert_eq!(golems.len(), 1);
    assert_eq!(golems[0].1, "iron_golem");
    let e = level.entity(golems[0].0);
    assert_eq!((e.x(), e.y(), e.z()), (5.5, -59.95, 5.5));
    assert!(iron_golem::is_player_created(e));
    for pos in BlockPos::between_closed(BlockPos::new(5, -60, 4), BlockPos::new(5, -58, 6)) {
        assert!(level.block_state(pos).is_air(), "{pos:?}");
    }
}

#[test]
fn iron_golem_patterns_need_air_beside_the_legs() {
    let mut level = build_level();
    place_iron_body(&mut level, BlockPos::new(5, -59, 5));
    level.set_block(BlockPos::new(4, -60, 5), blocks::STONE.default_state(), UPDATE_ALL);
    assert!(!carved_pumpkin_block::can_spawn_golem(&mut level, BlockPos::new(5, -58, 5)));
    level.set_block(BlockPos::new(5, -58, 5), blocks::CARVED_PUMPKIN.default_state(), UPDATE_ALL);
    assert!(golems(&level).is_empty());
    assert!(level.block_state(BlockPos::new(5, -58, 5)).is_block(blocks::CARVED_PUMPKIN));
}

#[test]
fn snow_patterns_win_over_iron_patterns() {
    let mut level = build_level();
    place_iron_body(&mut level, BlockPos::new(5, -59, 5));
    level.set_block(BlockPos::new(5, -59, 5), blocks::SNOW_BLOCK.default_state(), UPDATE_ALL);
    level.set_block(BlockPos::new(5, -60, 5), blocks::SNOW_BLOCK.default_state(), UPDATE_ALL);
    level.set_block(BlockPos::new(5, -58, 5), blocks::CARVED_PUMPKIN.default_state(), UPDATE_ALL);
    let golems = golems(&level);
    assert_eq!(golems.len(), 1);
    assert_eq!(golems[0].1, "snow_golem");
    assert!(level.block_state(BlockPos::new(4, -59, 5)).is_block(blocks::IRON_BLOCK));
}

#[test]
fn copper_blocks_let_dispensers_place_pumpkins() {
    let mut level = build_level();
    level.set_block(BlockPos::new(8, -60, 8), blocks::COPPER_BLOCK.default_state(), UPDATE_ALL);
    assert!(carved_pumpkin_block::can_spawn_golem(&mut level, BlockPos::new(8, -59, 8)));
    assert!(!carved_pumpkin_block::can_spawn_golem(&mut level, BlockPos::new(9, -59, 8)));
}
