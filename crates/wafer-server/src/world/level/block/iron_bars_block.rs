use wafer_core::{BlockPos, Direction};
use wafer_data::properties::{EAST, NORTH, SOUTH, WATERLOGGED, WEST};
use wafer_data::{Block, BlockState, SupportType, block_tags};

use super::cross_collision_block::{self, property_by_direction};
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::simple_waterlogged_block::{is_water_source, schedule_water_tick};
use super::{BlockBehaviour, is_exception_for_connection};
use crate::server::level::ServerLevel;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::{BlockGetter, LevelAccessor};

pub struct IronBarsBlock;

fn attaches_to(state: BlockState, face_solid: bool) -> bool {
    !is_exception_for_connection(state) && face_solid || state.block().is_instance_of("IronBarsBlock") || state.is(&block_tags::WALLS)
}

impl BlockBehaviour for IronBarsBlock {
    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        cross_collision_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        cross_collision_block::mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        let pos = ctx.clicked_pos();
        let water = is_water_source(level, pos);
        let (north, south, west, east) = (pos.north(), pos.south(), pos.west(), pos.east());
        let north_state = level.block_state(north);
        let south_state = level.block_state(south);
        let west_state = level.block_state(west);
        let east_state = level.block_state(east);
        let north_sturdy = level.is_face_sturdy_at(north, north_state, Direction::South, SupportType::Full);
        let south_sturdy = level.is_face_sturdy_at(south, south_state, Direction::North, SupportType::Full);
        let west_sturdy = level.is_face_sturdy_at(west, west_state, Direction::East, SupportType::Full);
        let east_sturdy = level.is_face_sturdy_at(east, east_state, Direction::West, SupportType::Full);
        Some(
            block
                .default_state()
                .set(NORTH, attaches_to(north_state, north_sturdy))
                .set(SOUTH, attaches_to(south_state, south_sturdy))
                .set(WEST, attaches_to(west_state, west_sturdy))
                .set(EAST, attaches_to(east_state, east_sturdy))
                .set(WATERLOGGED, water),
        )
    }

    fn update_shape(
        &self,
        state: BlockState,
        level: &mut dyn LevelAccessor,
        pos: BlockPos,
        direction: Direction,
        neighbour_pos: BlockPos,
        neighbour_state: BlockState,
    ) -> BlockState {
        schedule_water_tick(state, level, pos);
        if !direction.axis().is_horizontal() {
            return state;
        }
        let face_solid = level.is_face_sturdy_at(neighbour_pos, neighbour_state, direction.opposite(), SupportType::Full);
        state.set(property_by_direction(direction), attaches_to(neighbour_state, face_solid))
    }
}

#[cfg(test)]
mod tests {
    use wafer_core::BlockPos;
    use wafer_data::blocks;
    use wafer_data::properties::{EAST, NORTH, SOUTH, WEST};

    use crate::server::level::testing;
    use crate::world::level::BlockGetter;
    use crate::world::level::update_flags::UPDATE_ALL;

    #[test]
    fn panes_attach_to_bars_walls_and_solid_faces() {
        let mut level = testing::level();
        let pos = BlockPos::new(0, -60, 0);
        level.set_block(pos, blocks::WHITE_STAINED_GLASS_PANE.default_state(), UPDATE_ALL);
        level.set_block(pos.north(), blocks::IRON_BARS.default_state(), UPDATE_ALL);
        level.set_block(pos.east(), blocks::COBBLESTONE_WALL.default_state(), UPDATE_ALL);
        level.set_block(pos.south(), blocks::OAK_LEAVES.default_state(), UPDATE_ALL);
        level.set_block(pos.west(), blocks::OAK_PLANKS.default_state(), UPDATE_ALL);
        let pane = level.block_state(pos);
        assert!(pane.get(NORTH) && pane.get(EAST) && pane.get(WEST));
        assert!(!pane.get(SOUTH));
        assert!(level.block_state(pos.north()).get(SOUTH));
    }
}
