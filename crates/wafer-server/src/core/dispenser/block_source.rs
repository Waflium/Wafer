use wafer_core::{BlockPos, Direction, Vec3};
use wafer_data::BlockState;
use wafer_data::properties::FACING;

#[derive(Clone, Copy)]
pub struct BlockSource {
    pub pos: BlockPos,
    pub state: BlockState,
}

impl BlockSource {
    pub fn center(&self) -> Vec3 {
        Vec3::at_center_of(self.pos)
    }

    pub fn facing(&self) -> Direction {
        self.state.get(FACING)
    }
}
