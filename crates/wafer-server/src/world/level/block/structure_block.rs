use wafer_core::BlockPos;
use wafer_data::properties::StructureMode;
use wafer_data::{Block, BlockState};

use super::{BlockBehaviour, BlockHitResult};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;

pub struct StructureBlock;

impl StructureBlock {
    fn trigger(&self, level: &mut ServerLevel, pos: BlockPos) {
        let Some(mode) = level.structure_block_entity(pos).map(|entity| entity.mode) else { return };
        match mode {
            StructureMode::Save => {
                level.structure_block_entity_save_structure_to(pos, false);
            }
            StructureMode::Load => level.structure_block_entity_place_structure(pos),
            StructureMode::Corner => level.structure_block_entity_unload_structure(pos),
            StructureMode::Data => {}
        }
    }
}

impl BlockBehaviour for StructureBlock {
    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if level.structure_block_entity(pos).is_some() && level.can_use_game_master_blocks(player) { InteractionResult::SUCCESS } else { InteractionResult::Pass }
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, placer: Option<EntityId>, _item: ItemStack) {
        if let Some(placer) = placer
            && level.structure_block_entity(pos).is_some()
        {
            level.structure_block_entity_created_by(pos, placer);
        }
    }

    fn neighbor_changed(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _moved_by_piston: bool) {
        if level.structure_block_entity(pos).is_none() {
            return;
        }
        let should_trigger = level.has_neighbor_signal(pos);
        let structure_block = level.structure_block_entity(pos).unwrap();
        let is_powered = structure_block.powered;
        if should_trigger && !is_powered {
            structure_block.powered = true;
            self.trigger(level, pos);
        } else if !should_trigger && is_powered {
            structure_block.powered = false;
        }
    }
}
