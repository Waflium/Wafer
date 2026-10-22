use wafer_core::{BlockPos, Direction};
use wafer_data::{Block, BlockState, blocks};

use super::BlockBehaviour;
use super::falling_block;
use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK;
use crate::world::entity::EntityId;
use crate::world::entity::item::falling_block_entity;
use crate::world::level::gameevent::game_event;
use crate::world::level::{BlockGetter, LevelAccessor};

pub const TICK_DELAY: i32 = 2;

pub struct BrushableBlock {
    turns_into: Block,
    brush_sound: &'static str,
}

impl BrushableBlock {
    pub fn new(block: Block) -> BrushableBlock {
        if block == blocks::SUSPICIOUS_GRAVEL {
            BrushableBlock { turns_into: blocks::GRAVEL, brush_sound: "minecraft:item.brush.brushing.gravel" }
        } else {
            BrushableBlock { turns_into: blocks::SAND, brush_sound: "minecraft:item.brush.brushing.sand" }
        }
    }

    pub fn get_turns_into(&self) -> Block {
        self.turns_into
    }

    pub fn get_brush_sound(&self) -> &'static str {
        self.brush_sound
    }
}

pub fn of(block: Block) -> Option<BrushableBlock> {
    block.is_instance_of("BrushableBlock").then(|| BrushableBlock::new(block))
}

impl BlockBehaviour for BrushableBlock {
    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, _old_state: BlockState, _moved_by_piston: bool) {
        level.schedule_tick(pos, state.block(), TICK_DELAY);
    }

    fn update_shape(&self, state: BlockState, level: &mut dyn LevelAccessor, pos: BlockPos, _: Direction, _: BlockPos, _: BlockState) -> BlockState {
        level.schedule_tick(pos, state.block(), TICK_DELAY);
        state
    }

    fn tick(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos) {
        level.brushable_check_reset(pos);
        if falling_block::is_free(level.block_state(pos.below())) && pos.y >= level.chunks.min_y() {
            let entity = falling_block_entity::fall(level, pos, state);
            level.falling_block(entity).disable_drop();
        }
    }

    fn on_broken_after_fall(&self, level: &mut ServerLevel, _pos: BlockPos, entity: EntityId) {
        let center_of_entity = level.entity(entity).bounding_box().center();
        let block_state = level.falling_block(entity).get_block_state();
        level.level_event(None, PARTICLES_AND_SOUND_DESTROY_BLOCK, BlockPos::containing_vec(center_of_entity), block_state.id() as i32);
        level.game_event_from_entity(Some(entity), game_event::BLOCK_DESTROY, center_of_entity);
    }
}
