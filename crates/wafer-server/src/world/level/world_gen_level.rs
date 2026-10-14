use wafer_core::BlockPos;
use wafer_data::{BlockState, FluidState};

use super::chunk::ChunkGenerator;
use super::levelgen::material::WorldGenerationContext;
use super::server_level_accessor::ServerLevelAccessor;

pub trait WorldGenLevel: ServerLevelAccessor {
    fn get_seed(&self) -> i64;

    fn ensure_can_write(&self, pos: BlockPos) -> bool;

    fn generator(&self) -> &ChunkGenerator;

    fn generation(&self) -> WorldGenerationContext;

    fn get_block_state(&mut self, pos: BlockPos) -> BlockState;

    fn get_fluid_state(&mut self, pos: BlockPos) -> FluidState {
        self.get_block_state(pos).fluid_state()
    }

    fn get_height_of_level(&self) -> i32;

    fn get_max_y(&self) -> i32 {
        self.get_min_y() + self.get_height_of_level() - 1
    }

    fn is_outside_build_height(&self, y: i32) -> bool {
        y < self.get_min_y() || y >= self.get_min_y() + self.get_height_of_level()
    }

    fn set_block_and_update(&mut self, pos: BlockPos, state: BlockState) -> bool {
        self.set_block(pos, state, super::update_flags::UPDATE_ALL)
    }

    fn set_section_block_state(&mut self, pos: BlockPos, state: BlockState);

    fn destroy_block(&mut self, pos: BlockPos, drop_resources: bool) -> bool;
}
