use wafer_core::{BlockPos, Vec3};

#[derive(Clone, Copy, Debug)]
pub struct BlockPosTracker {
    block_pos: BlockPos,
    center_position: Vec3,
}

impl BlockPosTracker {
    pub fn new(block_pos: BlockPos) -> BlockPosTracker {
        BlockPosTracker { block_pos, center_position: Vec3::at_center_of(block_pos) }
    }

    pub fn of_vec(vec: Vec3) -> BlockPosTracker {
        BlockPosTracker { block_pos: BlockPos::containing_vec(vec), center_position: vec }
    }

    pub fn current_position(&self) -> Vec3 {
        self.center_position
    }

    pub fn current_block_position(&self) -> BlockPos {
        self.block_pos
    }
}
