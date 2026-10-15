use std::sync::LazyLock;

use wafer_core::BlockPos;
use wafer_data::properties::HORIZONTAL_FACING;
use wafer_data::{Block, BlockState, block_tags, blocks};

use super::BlockBehaviour;
use super::horizontal_directional_block;
use super::mirror::Mirror;
use super::rotation::Rotation;
use super::state::pattern::block_pattern::{BlockInWorldPredicate, BlockPattern, BlockPatternBuilder, BlockPatternMatch, has_state};
use crate::advancements::triggers::criteria_triggers;
use crate::server::level::ServerLevel;
use crate::server::level::events::PARTICLES_AND_SOUND_DESTROY_BLOCK;
use crate::world::entity::LevelEntity;
use crate::world::entity::animal::golem::iron_golem::{self, IronGolem};
use crate::world::entity::animal::golem::snow_golem::SnowGolem;
use crate::world::entity::mob::Mob;
use crate::world::item::context::BlockPlaceContext;
use crate::world::level::BlockGetter;
use crate::world::level::update_flags::UPDATE_CLIENTS;

pub struct CarvedPumpkinBlock;

fn pumpkins_predicate() -> BlockInWorldPredicate {
    has_state(|input| input.is_block(blocks::CARVED_PUMPKIN) || input.is_block(blocks::JACK_O_LANTERN))
}

static SNOW_GOLEM_BASE: LazyLock<BlockPattern> =
    LazyLock::new(|| BlockPatternBuilder::start().aisle(&[" ", "#", "#"]).where_('#', has_state(|state| state.is_block(blocks::SNOW_BLOCK))).build());

static SNOW_GOLEM_FULL: LazyLock<BlockPattern> = LazyLock::new(|| {
    BlockPatternBuilder::start()
        .aisle(&["^", "#", "#"])
        .where_('^', pumpkins_predicate())
        .where_('#', has_state(|state| state.is_block(blocks::SNOW_BLOCK)))
        .build()
});

static IRON_GOLEM_BASE: LazyLock<BlockPattern> = LazyLock::new(|| {
    BlockPatternBuilder::start()
        .aisle(&["~ ~", "###", "~#~"])
        .where_('#', has_state(|state| state.is_block(blocks::IRON_BLOCK)))
        .where_('~', has_state(|state| state.is_air()))
        .build()
});

static IRON_GOLEM_FULL: LazyLock<BlockPattern> = LazyLock::new(|| {
    BlockPatternBuilder::start()
        .aisle(&["~^~", "###", "~#~"])
        .where_('^', pumpkins_predicate())
        .where_('#', has_state(|state| state.is_block(blocks::IRON_BLOCK)))
        .where_('~', has_state(|state| state.is_air()))
        .build()
});

static COPPER_GOLEM_BASE: LazyLock<BlockPattern> =
    LazyLock::new(|| BlockPatternBuilder::start().aisle(&[" ", "#"]).where_('#', has_state(|state| state.is(&block_tags::COPPER))).build());

pub fn can_spawn_golem(level: &mut ServerLevel, top_pos: BlockPos) -> bool {
    SNOW_GOLEM_BASE.find(level, top_pos).is_some() || IRON_GOLEM_BASE.find(level, top_pos).is_some() || COPPER_GOLEM_BASE.find(level, top_pos).is_some()
}

fn try_spawn_golem(level: &mut ServerLevel, top_pos: BlockPos) {
    if let Some(snow_golem_match) = SNOW_GOLEM_FULL.find(level, top_pos) {
        let snow_golem = SnowGolem::create();
        let spawn_pos = snow_golem_match.get_block(0, 2, 0);
        spawn_golem_in_world(level, &snow_golem_match, snow_golem, spawn_pos);
        return;
    }
    if let Some(iron_golem_match) = IRON_GOLEM_FULL.find(level, top_pos) {
        let mut iron_golem = IronGolem::create();
        iron_golem::set_player_created(&mut iron_golem.entity, true);
        let spawn_pos = iron_golem_match.get_block(1, 2, 0);
        spawn_golem_in_world(level, &iron_golem_match, iron_golem, spawn_pos);
    }
}

fn spawn_golem_in_world(level: &mut ServerLevel, golem_match: &BlockPatternMatch, mut golem: Mob, spawn_pos: BlockPos) {
    clear_pattern_blocks(level, golem_match);
    golem.entity.snap_to(spawn_pos.x as f64 + 0.5, spawn_pos.y as f64 + 0.05, spawn_pos.z as f64 + 0.5, 0.0, 0.0);
    let id = golem.entity.id;
    level.add_fresh_entity(LevelEntity::Mob(Box::new(golem)));
    let area = level.entity(id).bounding_box().inflate(5.0, 5.0, 5.0);
    let players: Vec<_> = level.get_entities(None, &area).into_iter().filter(|&entity| level.is_player(entity)).collect();
    for player in players {
        criteria_triggers::SUMMONED_ENTITY.trigger(level, player, id);
    }
    update_pattern_blocks(level, golem_match);
}

pub fn clear_pattern_blocks(level: &mut ServerLevel, golem_match: &BlockPatternMatch) {
    for x in 0..golem_match.get_width() as i32 {
        for y in 0..golem_match.get_height() as i32 {
            let pos = golem_match.get_block(x, y, 0);
            let state = level.block_state(pos);
            level.set_block(pos, blocks::AIR.default_state(), UPDATE_CLIENTS);
            level.level_event(None, PARTICLES_AND_SOUND_DESTROY_BLOCK, pos, state.id() as i32);
        }
    }
}

pub fn update_pattern_blocks(level: &mut ServerLevel, golem_match: &BlockPatternMatch) {
    for x in 0..golem_match.get_width() as i32 {
        for y in 0..golem_match.get_height() as i32 {
            let pos = golem_match.get_block(x, y, 0);
            level.update_neighbors_at(pos, blocks::AIR);
        }
    }
}

impl BlockBehaviour for CarvedPumpkinBlock {
    fn on_place(&self, state: BlockState, level: &mut ServerLevel, pos: BlockPos, old_state: BlockState, _moved_by_piston: bool) {
        if !old_state.is_block(state.block()) {
            try_spawn_golem(level, pos);
        }
    }

    fn rotate(&self, state: BlockState, rotation: Rotation) -> BlockState {
        horizontal_directional_block::rotate(state, rotation)
    }

    fn mirror(&self, state: BlockState, mirror: Mirror) -> BlockState {
        horizontal_directional_block::mirror(state, mirror)
    }

    fn get_state_for_placement(&self, block: Block, ctx: &BlockPlaceContext, _level: &mut ServerLevel) -> Option<BlockState> {
        Some(block.default_state().set(HORIZONTAL_FACING, ctx.horizontal_direction().opposite()))
    }
}
