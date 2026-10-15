use wafer_core::{BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::behavior::block_pos_tracker::BlockPosTracker;
use crate::world::entity::ai::behavior::entity_tracker::EntityTracker;
use crate::world::entity::ai::behavior::position_tracker::PositionTracker;

#[derive(Clone, Debug)]
pub struct WalkTarget {
    target: PositionTracker,
    speed_modifier: f32,
    close_enough_dist: i32,
}

impl WalkTarget {
    pub fn new(target: PositionTracker, speed_modifier: f32, close_enough_dist: i32) -> WalkTarget {
        WalkTarget { target, speed_modifier, close_enough_dist }
    }

    pub fn of_block_pos(target: BlockPos, speed_modifier: f32, close_enough_dist: i32) -> WalkTarget {
        WalkTarget::new(PositionTracker::BlockPos(BlockPosTracker::new(target)), speed_modifier, close_enough_dist)
    }

    pub fn of_vec(target: Vec3, speed_modifier: f32, close_enough_dist: i32) -> WalkTarget {
        WalkTarget::of_block_pos(BlockPos::containing_vec(target), speed_modifier, close_enough_dist)
    }

    pub fn of_entity(level: &ServerLevel, target: EntityId, speed_modifier: f32, close_enough_dist: i32) -> WalkTarget {
        WalkTarget::new(PositionTracker::Entity(EntityTracker::new(level, target, false)), speed_modifier, close_enough_dist)
    }

    pub fn get_target(&self) -> &PositionTracker {
        &self.target
    }

    pub fn get_speed_modifier(&self) -> f32 {
        self.speed_modifier
    }

    pub fn get_close_enough_dist(&self) -> i32 {
        self.close_enough_dist
    }
}
