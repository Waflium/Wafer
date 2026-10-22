use wafer_core::{BlockPos, mth};
use wafer_data::Tag;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;

pub struct TryFindLiquidGoal {
    fluid_tag: &'static Tag,
}

impl TryFindLiquidGoal {
    pub fn new(fluid_tag: &'static Tag) -> TryFindLiquidGoal {
        TryFindLiquidGoal { fluid_tag }
    }

    pub fn can_use(&self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let e = level.entity(mob);
        let (on_ground, pos) = (e.on_ground, e.block_position());
        on_ground && !level.fluid_state(pos).is(self.fluid_tag)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let e = level.entity(mob);
        let from = BlockPos::new(mth::floor(e.x() - 2.0), mth::floor(e.y() - 2.0), mth::floor(e.z() - 2.0));
        let to = BlockPos::new(mth::floor(e.x() + 2.0), e.block_position().y, mth::floor(e.z() + 2.0));
        let fluid_pos = BlockPos::between_closed(from, to).find(|&pos| level.fluid_state(pos).is(self.fluid_tag));
        if let Some(fluid_pos) = fluid_pos {
            level.set_wanted_position(mob, fluid_pos.x as f64, fluid_pos.y as f64, fluid_pos.z as f64, 1.0);
        }
    }
}
