use wafer_core::BlockPos;
use wafer_data::fluid_tags;

use super::entity_type::EntityType;
use crate::world::level::block::state::block_behaviour;
use crate::world::level::natural_spawner;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SpawnPlacementType {
    NoRestrictions,
    InWater,
    InLava,
    OnGround,
}

impl SpawnPlacementType {
    pub fn is_spawn_position_ok(self, level: &mut (impl ServerLevelAccessor + ?Sized), block_pos: BlockPos, entity_type: Option<&EntityType>) -> bool {
        match self {
            SpawnPlacementType::NoRestrictions => true,
            SpawnPlacementType::InWater => match entity_type {
                Some(_) if level.is_within_world_border(block_pos) => {
                    let above = block_pos.above();
                    level.fluid_state(block_pos).is(&fluid_tags::WATER) && !level.block_state(above).is_redstone_conductor()
                }
                _ => false,
            },
            SpawnPlacementType::InLava => match entity_type {
                Some(_) if level.is_within_world_border(block_pos) => level.fluid_state(block_pos).is(&fluid_tags::LAVA),
                _ => false,
            },
            SpawnPlacementType::OnGround => match entity_type {
                Some(entity_type) if level.is_within_world_border(block_pos) => {
                    let above = block_pos.above();
                    let below = block_pos.below();
                    let below_state = level.block_state(below);
                    if !block_behaviour::is_valid_spawn(below_state, entity_type) {
                        return false;
                    }
                    is_valid_empty_spawn_block(level, block_pos, entity_type) && is_valid_empty_spawn_block(level, above, entity_type)
                }
                _ => false,
            },
        }
    }

    pub fn adjust_spawn_position(self, level: &mut (impl ServerLevelAccessor + ?Sized), candidate: BlockPos) -> BlockPos {
        match self {
            SpawnPlacementType::OnGround => {
                let below = candidate.below();
                if level.block_state(below).is_pathfindable_land() { below } else { candidate }
            }
            _ => candidate,
        }
    }
}

fn is_valid_empty_spawn_block(level: &mut (impl ServerLevelAccessor + ?Sized), block_pos: BlockPos, entity_type: &EntityType) -> bool {
    let block_state = level.block_state(block_pos);
    natural_spawner::is_valid_empty_spawn_block(block_state, block_state.fluid_state(), entity_type)
}
