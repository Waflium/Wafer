use wafer_core::BlockPos;
use wafer_data::BlockState;

use crate::server::level::ServerLevel;
use crate::world::level::BlockGetter;
use crate::world::level::block::entity::BlockEntity;

pub struct BlockInWorld<'a> {
    level: &'a mut ServerLevel,
    pos: BlockPos,
    load_chunks: bool,
}

impl<'a> BlockInWorld<'a> {
    pub fn new(level: &'a mut ServerLevel, pos: BlockPos, load_chunks: bool) -> BlockInWorld<'a> {
        BlockInWorld { level, pos, load_chunks }
    }

    pub fn get_state(&mut self) -> Option<BlockState> {
        (self.load_chunks || self.level.has_chunk_at(self.pos)).then(|| self.level.block_state(self.pos))
    }

    pub fn get_entity(&mut self) -> Option<&mut BlockEntity> {
        self.level.get_block_entity(self.pos)
    }
}
