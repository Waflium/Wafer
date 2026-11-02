use wafer_core::BlockPos;
use wafer_data::BlockState;

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;

pub struct SmithingTableBlock;

impl BlockBehaviour for SmithingTableBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        crate::server::main_thread_frames::within(&["net.minecraft.world.level.block.SmithingTableBlock.useWithoutItem(SmithingTableBlock.java:31)"], || level.open_menu(player, crate::world::inventory::menu_provider::MenuProvider::Smithing(pos)));
        level.award_stat(player, crate::stats::stats::INTERACT_WITH_SMITHING_TABLE, 1);
        InteractionResult::SUCCESS
    }

    fn get_menu_provider(&self, _state: BlockState, _level: &mut ServerLevel, pos: BlockPos) -> Option<crate::world::inventory::menu_provider::MenuProvider> {
        Some(crate::world::inventory::menu_provider::MenuProvider::Smithing(pos))
    }
}
