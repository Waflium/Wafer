use std::sync::LazyLock;

use wafer_core::{Axis, BlockPos};
use wafer_data::{Block, BlockState, block_tags, blocks, items};

use super::BlockBehaviour;
use super::carved_pumpkin_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::skull_block::SkullBlock;
use super::state::pattern::block_pattern::{BlockPattern, BlockPatternBuilder, has_state};
use super::wall_skull_block::WallSkullBlock;
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::world::difficulty::Difficulty;
use crate::world::entity::boss::wither::wither_boss::WitherBoss;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::item::ItemStack;
use crate::world::item::context::BlockPlaceContext;

static WITHER_PATTERN_FULL: LazyLock<BlockPattern> = LazyLock::new(|| {
    BlockPatternBuilder::start()
        .aisle(&["^^^", "###", "~#~"])
        .where_('#', has_state(|state| state.is(&block_tags::WITHER_SUMMON_BASE_BLOCKS)))
        .where_('^', has_state(|state| state.is_block(blocks::WITHER_SKELETON_SKULL) || state.is_block(blocks::WITHER_SKELETON_WALL_SKULL)))
        .where_('~', has_state(|state| state.is_air()))
        .build()
});

static WITHER_PATTERN_BASE: LazyLock<BlockPattern> = LazyLock::new(|| {
    BlockPatternBuilder::start()
        .aisle(&["   ", "###", "~#~"])
        .where_('#', has_state(|state| state.is(&block_tags::WITHER_SUMMON_BASE_BLOCKS)))
        .where_('~', has_state(|state| state.is_air()))
        .build()
});

pub fn check_spawn(level: &mut ServerLevel, pos: BlockPos) {
    let Some(block_entity) = level.get_block_entity(pos) else { return };
    if !matches!(block_entity.kind, crate::world::level::block::entity::BlockEntityKind::Skull(_)) {
        return;
    }
    let block_state = block_entity.block_state();
    let correct_block = block_state.is_block(blocks::WITHER_SKELETON_SKULL) || block_state.is_block(blocks::WITHER_SKELETON_WALL_SKULL);
    if !correct_block || pos.y < level.chunks.min_y() || level.difficulty == Difficulty::Peaceful {
        return;
    }
    let Some(pattern_match) = WITHER_PATTERN_FULL.find(level, pos) else { return };
    let mut wither_boss = WitherBoss::create();
    carved_pumpkin_block::clear_pattern_blocks(level, &pattern_match);
    let spawn_pos = pattern_match.get_block(1, 2, 0);
    let rot = if pattern_match.get_forwards().axis() == Axis::X { 0.0 } else { 90.0 };
    wither_boss.entity.snap_to(spawn_pos.x as f64 + 0.5, spawn_pos.y as f64 + 0.55, spawn_pos.z as f64 + 0.5, rot, 0.0);
    wither_boss.living.y_body_rot = rot;
    WitherBoss::make_invulnerable(&mut wither_boss);
    let id = wither_boss.entity.id;
    let area = wither_boss.entity.bounding_box().inflate_all(50.0);
    let players: Vec<EntityId> = level.get_entities_matching(None, &area, |level, entity| level.is_player(entity));
    level.add_fresh_entity(LevelEntity::Mob(Box::new(wither_boss)));
    for player in players {
        criteria_triggers::SUMMONED_ENTITY.trigger(level, player, id);
    }
    carved_pumpkin_block::update_pattern_blocks(level, &pattern_match);
}

pub fn can_spawn_mob(level: &mut ServerLevel, pos: BlockPos, item_stack: &ItemStack) -> bool {
    if item_stack.is(items::WITHER_SKELETON_SKULL) && pos.y >= level.chunks.min_y() + 2 && level.difficulty != Difficulty::Peaceful {
        WITHER_PATTERN_BASE.find(level, pos).is_some()
    } else {
        false
    }
}

pub struct WitherSkullBlock;

impl BlockBehaviour for WitherSkullBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        SkullBlock.get_state_for_placement(block, ctx, level)
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
        SkullBlock.neighbor_changed(state, level, pos, block, moved_by_piston);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        SkullBlock.rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        SkullBlock.mirror(state, mirror)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        check_spawn(level, pos);
    }
}

pub struct WitherWallSkullBlock;

impl BlockBehaviour for WitherWallSkullBlock {
    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, level: &mut ServerLevel) -> Option<BlockState> {
        WallSkullBlock.get_state_for_placement(block, ctx, level)
    }

    fn neighbor_changed(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, block: Block, moved_by_piston: bool) {
        WallSkullBlock.neighbor_changed(state, level, pos, block, moved_by_piston);
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        WallSkullBlock.rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        WallSkullBlock.mirror(state, mirror)
    }

    fn set_placed_by(&self, level: &mut ServerLevel, pos: BlockPos, _state: BlockState, _placer: Option<EntityId>, _item: ItemStack) {
        check_spawn(level, pos);
    }
}

