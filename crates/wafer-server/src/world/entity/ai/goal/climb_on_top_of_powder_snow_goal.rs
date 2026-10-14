use wafer_data::{blocks, entity_type_tags};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::phys::shapes::collision_context::CollisionContext;

pub struct ClimbOnTopOfPowderSnowGoal;

impl ClimbOnTopOfPowderSnowGoal {
    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        let in_powder_snow = e.was_in_powder_snow || e.is_in_powder_snow;
        if in_powder_snow && entity_type_tags::POWDER_SNOW_WALKABLE_MOBS.contains(e.entity_type as usize) {
            let above = e.block_position().above();
            let above_block_state = level.block_state(above);
            above_block_state.is_block(blocks::POWDER_SNOW) || CollisionContext::EMPTY.get_collision_shape(above_block_state, above).is_empty()
        } else {
            false
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").ai.jump_control.jump();
    }
}
