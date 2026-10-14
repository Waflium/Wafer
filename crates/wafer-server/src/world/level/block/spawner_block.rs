use wafer_core::BlockPos;
use wafer_core::random::RandomSource;
use wafer_data::BlockState;

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;

pub struct SpawnerBlock;

impl BlockBehaviour for SpawnerBlock {
    fn trigger_event(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, b0: i32, b1: i32) -> bool {
        level.get_block_entity(pos).is_some_and(|e| e.trigger_event(b0, b1))
    }

    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _tool: Option<ItemStack>, drop_experience: bool) {
        if drop_experience {
            let magic_count = 15 + level.random.next_int_bounded(15) + level.random.next_int_bounded(15);
            level.pop_experience(pos, magic_count);
        }
    }
}
