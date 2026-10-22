use wafer_core::BlockPos;
use wafer_data::BlockState;
use wafer_data::properties::BLOOM;

use super::BlockBehaviour;
use super::drop_experience_block::XpRange;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::level::update_flags::UPDATE_ALL;

pub struct SculkCatalystBlock;

impl BlockBehaviour for SculkCatalystBlock {
    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        if state.get(BLOOM) {
            level.set_block(pos, state.set(BLOOM, false), UPDATE_ALL);
        }
    }

    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, drop_experience: bool) {
        if drop_experience {
            level.try_drop_experience(pos, tool.as_ref(), XpRange::Constant(5));
        }
    }
}
