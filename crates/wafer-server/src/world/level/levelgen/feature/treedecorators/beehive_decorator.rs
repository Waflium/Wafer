use crate::world::level::world_gen_level::WorldGenLevel;
use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Direction};
use wafer_data::block_entity_types;
use wafer_data::blocks;
use wafer_data::properties::HORIZONTAL_FACING;

use super::tree_decorator::Context;
use crate::util;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::block::entity::beehive_block_entity::Occupant;

const WORLDGEN_FACING: Direction = Direction::South;
const SPAWN_DIRECTIONS: [Direction; 3] = [Direction::East, Direction::South, Direction::West];

pub fn fields() -> Vec<crate::world::item::component::codec::Field> {
    use crate::world::item::component::types::{self, req};
    vec![req("probability", types::float(types::codec_float_range(0.0, 1.0)))]
}

pub fn place<R: RandomSource>(probability: f32, context: &mut Context<impl WorldGenLevel, R>) {
    if context.logs.is_empty() {
        return;
    }
    if context.random.next_float() >= probability {
        return;
    }
    let hive_y = if !context.leaves.is_empty() {
        (context.leaves[0].y - 1).max(context.logs[0].y + 1)
    } else {
        (context.logs[0].y + 1 + context.random.next_int_bounded(3)).min(context.logs[context.logs.len() - 1].y)
    };
    let mut hive_placements: Vec<BlockPos> = context.logs.iter().filter(|pos| pos.y == hive_y).flat_map(|&pos| SPAWN_DIRECTIONS.map(|dir| pos.relative(dir))).collect();
    if hive_placements.is_empty() {
        return;
    }
    util::shuffle(&mut hive_placements, context.random);
    let hive_pos = hive_placements.into_iter().find(|&pos| context.is_air(pos) && context.is_air(pos.relative(WORLDGEN_FACING)));
    let Some(hive_pos) = hive_pos else { return };
    context.set_block(hive_pos, blocks::BEE_NEST.default_state().set(HORIZONTAL_FACING, WORLDGEN_FACING));
    if let Some(entity) = context.level.get_block_entity(hive_pos)
        && entity.ty() == block_entity_types::BEEHIVE
        && let BlockEntityKind::Beehive(beehive) = &mut entity.kind
    {
        let num_bees = 2 + context.random.next_int_bounded(2);
        for _ in 0..num_bees {
            beehive.store_bee(Occupant::create(context.random.next_int_bounded(599)));
        }
    }
}
