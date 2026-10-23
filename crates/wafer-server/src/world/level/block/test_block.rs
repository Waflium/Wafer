use wafer_core::BlockPos;
use wafer_data::properties::{TEST_BLOCK_MODE, TestBlockMode};
use wafer_data::{Block, BlockState};

use super::{BlockBehaviour, BlockHitResult, BlockPlaceContext};
use crate::server::level::ServerLevel;
use crate::world::InteractionResult;
use crate::world::entity::EntityId;
use crate::world::item::ItemStack;
use crate::world::item::component as dc;
use crate::world::level::LevelReader;
use crate::world::level::block::entity::BlockEntityKind;
use crate::world::level::redstone::orientation::Orientation;
use crate::world::level::signal;

pub struct TestBlock;

pub fn set_mode_on_stack(item_stack: &mut ItemStack, mode: TestBlockMode) {
    let mut properties = match item_stack.get(dc::BLOCK_STATE) {
        Some(wafer_protocol::nbt::Nbt::Compound(properties)) => properties.clone(),
        _ => Vec::new(),
    };
    properties.retain(|(name, _)| name != "mode");
    properties.push(("mode".into(), wafer_protocol::nbt::Nbt::String(mode.name().into())));
    item_stack.set(dc::BLOCK_STATE, Some(wafer_protocol::nbt::Nbt::Compound(properties)));
}

impl BlockBehaviour for TestBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        let to_place = block.default_state();
        let mode = ctx.item.get(dc::BLOCK_STATE).and_then(|properties| properties.get("mode")).and_then(|mode| mode.as_str()).and_then(TestBlockMode::by_name);
        Some(match mode {
            Some(mode) => to_place.set(TEST_BLOCK_MODE, mode),
            None => to_place,
        })
    }

    fn use_without_item(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, player: EntityId, _hit: &BlockHitResult) -> InteractionResult {
        if level.test_block_entity(pos).is_none() {
            return InteractionResult::Pass;
        }
        if !level.can_use_game_master_blocks(player) {
            return InteractionResult::Pass;
        }
        InteractionResult::SUCCESS
    }

    fn tick(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        level.test_block_entity_reset(pos);
    }

    fn neighbor_changed(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, _block: Block, _orientation: Option<Orientation>, _moved_by_piston: bool) {
        let Some(test_block) = level.test_block_entity(pos) else { return };
        if test_block.get_mode() == TestBlockMode::Start {
            return;
        }
        let is_powered = test_block.is_powered();
        let should_trigger = signal::has_neighbor_signal(level, pos);
        if should_trigger && !is_powered {
            level.test_block_entity(pos).unwrap().set_powered(true);
            level.test_block_entity_trigger(pos);
        } else if !should_trigger && is_powered {
            level.test_block_entity(pos).unwrap().set_powered(false);
        }
    }

    fn own_signal(&self, state: BlockState, level: &mut dyn LevelReader, pos: BlockPos) -> i32 {
        if state.get(TEST_BLOCK_MODE) != TestBlockMode::Start {
            return 0;
        }
        match level.get_block_entity(pos).map(|block_entity| &block_entity.kind) {
            Some(BlockEntityKind::TestBlock(test_block)) if test_block.is_powered() => 15,
            _ => 0,
        }
    }

    fn get_clone_item_stack(&self, _level: &ServerLevel, _pos: BlockPos, state: BlockState, _include_data: bool) -> ItemStack {
        let mut item_stack = ItemStack::new(state.block().item(), 1);
        set_mode_on_stack(&mut item_stack, state.get(TEST_BLOCK_MODE));
        item_stack
    }
}
