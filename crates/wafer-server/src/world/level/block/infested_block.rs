use wafer_core::BlockPos;
use wafer_data::{Block, BlockState, blocks, game_rule};

use super::BlockBehaviour;
use crate::server::level::ServerLevel;
use crate::world::item::ItemStack;
use crate::world::item::enchantment::enchantment_helper;

const BLOCK_BY_HOST_BLOCK: [(Block, Block); 7] = [
    (blocks::STONE, blocks::INFESTED_STONE),
    (blocks::COBBLESTONE, blocks::INFESTED_COBBLESTONE),
    (blocks::STONE_BRICKS, blocks::INFESTED_STONE_BRICKS),
    (blocks::MOSSY_STONE_BRICKS, blocks::INFESTED_MOSSY_STONE_BRICKS),
    (blocks::CRACKED_STONE_BRICKS, blocks::INFESTED_CRACKED_STONE_BRICKS),
    (blocks::CHISELED_STONE_BRICKS, blocks::INFESTED_CHISELED_STONE_BRICKS),
    (blocks::DEEPSLATE, blocks::INFESTED_DEEPSLATE),
];

pub fn is_infested_block(block_state: BlockState) -> bool {
    BLOCK_BY_HOST_BLOCK.iter().any(|&(_, infested)| block_state.is_block(infested))
}

pub fn is_compatible_host_block(block_state: BlockState) -> bool {
    BLOCK_BY_HOST_BLOCK.iter().any(|&(host, _)| block_state.is_block(host))
}

pub fn get_host_block(infested_state: BlockState) -> Block {
    BLOCK_BY_HOST_BLOCK.iter().find(|&&(_, infested)| infested_state.is_block(infested)).expect("an infested block").0
}

pub fn infested_state_by_host(host_state: BlockState) -> BlockState {
    let infested = BLOCK_BY_HOST_BLOCK.iter().find(|&&(host, _)| host_state.is_block(host)).expect("a host block").1;
    get_new_state_with_properties(host_state, infested.default_state())
}

pub fn host_state_by_infested(infested_state: BlockState) -> BlockState {
    get_new_state_with_properties(infested_state, get_host_block(infested_state).default_state())
}

fn get_new_state_with_properties(old_state: BlockState, new_state: BlockState) -> BlockState {
    new_state.with_properties_of(old_state)
}

pub fn spawn_infestation(level: &mut ServerLevel, pos: BlockPos) {
    if let Some(mut silverfish) = ServerLevel::create_entity("minecraft:silverfish") {
        silverfish.entity_mut().snap_to(pos.x as f64 + 0.5, pos.y as f64, pos.z as f64 + 0.5, 0.0, 0.0);
        let id = silverfish.entity().id;
        level.add_fresh_entity(silverfish);
        level.spawn_anim(id);
    }
}

pub fn spawn_after_break(level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>) {
    if level.game_rules.get_bool(game_rule::BLOCK_DROPS) && !tool.is_some_and(|tool| enchantment_helper::has_tag(&tool, "minecraft:prevents_infested_spawns")) {
        spawn_infestation(level, pos);
    }
}

pub struct InfestedBlock;

impl BlockBehaviour for InfestedBlock {
    fn spawn_after_break(&self, _state: BlockState, level: &mut ServerLevel, pos: BlockPos, tool: Option<ItemStack>, _drop_experience: bool) {
        spawn_after_break(level, pos, tool);
    }
}
