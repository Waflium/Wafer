use wafer_core::{BlockPos, mth};
use wafer_data::{BlockState, blocks};

use crate::server::level::ServerLevel;
use crate::world::entity::entity_spawn_reason::EntitySpawnReason;
use crate::world::entity::entity_type::EntityType;
use crate::world::entity::{EntityId, LevelEntity, mob};
use crate::world::level::BlockGetter;
use crate::world::level::server_level_accessor::ServerLevelAccessor;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Strategy {
    LegacyIronGolem,
}

impl Strategy {
    fn can_spawn_on(self, block_state: BlockState, above_state: BlockState) -> bool {
        match self {
            Strategy::LegacyIronGolem => {
                let block = block_state.block();
                if block == blocks::COBWEB
                    || block == blocks::CACTUS
                    || block == blocks::GLASS_PANE
                    || block.is_instance_of("StainedGlassPaneBlock")
                    || block.is_instance_of("StainedGlassBlock")
                    || block.is_instance_of("LeavesBlock")
                    || block == blocks::CONDUIT
                    || block == blocks::ICE
                    || block == blocks::TNT
                    || block == blocks::GLOWSTONE
                    || block == blocks::BEACON
                    || block == blocks::SEA_LANTERN
                    || block == blocks::FROSTED_ICE
                    || block == blocks::TINTED_GLASS
                    || block == blocks::GLASS
                {
                    false
                } else {
                    (above_state.is_air() || above_state.liquid()) && (block_state.is_solid() || block == blocks::POWDER_SNOW)
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub fn try_spawn_mob(
    level: &mut ServerLevel,
    entity_type: &EntityType,
    spawn_reason: EntitySpawnReason,
    start: BlockPos,
    spawn_attempts: i32,
    spawn_range_xz: i32,
    spawn_range_y: i32,
    strategy: Strategy,
    check_collisions: bool,
) -> Option<EntityId> {
    for _ in 0..spawn_attempts {
        let dx = mth::random_between_inclusive(&mut level.random, -spawn_range_xz, spawn_range_xz);
        let dz = mth::random_between_inclusive(&mut level.random, -spawn_range_xz, spawn_range_xz);
        let mut search_pos = start.offset(dx, spawn_range_y, dz);
        if level.world_border.is_within_bounds_pos(search_pos)
            && move_to_possible_spawn_position(level, spawn_range_y, &mut search_pos, strategy)
            && (!check_collisions || level.no_collision(&entity_type.get_spawn_aabb(search_pos.x as f64 + 0.5, search_pos.y as f64, search_pos.z as f64 + 0.5)))
            && let Some(entity) = level.create_entity_type_from_stack(entity_type.get_key(), None, search_pos, spawn_reason, false, false)
        {
            let LevelEntity::Mob(mob_entity) = entity else { continue };
            if mob::check_spawn_rules(&mob_entity, level, spawn_reason) && mob::check_spawn_obstruction(&mob_entity, level) {
                let id = mob_entity.entity.id;
                level.add_fresh_entity_with_passengers(LevelEntity::Mob(mob_entity));
                level.play_ambient_sound(id);
                return Some(id);
            }
        }
    }
    None
}

fn move_to_possible_spawn_position(level: &mut ServerLevel, spawn_range_y: i32, search_pos: &mut BlockPos, strategy: Strategy) -> bool {
    let mut above_state = level.block_state(*search_pos);
    let mut y = spawn_range_y;
    while y >= -spawn_range_y {
        *search_pos = search_pos.below();
        let current_state = level.block_state(*search_pos);
        if strategy.can_spawn_on(current_state, above_state) {
            *search_pos = search_pos.above();
            return true;
        }
        above_state = current_state;
        y -= 1;
    }
    false
}
