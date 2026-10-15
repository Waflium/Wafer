use wafer_core::{BlockPos, Vec3};

use super::block_pos_tracker::BlockPosTracker;
use super::entity_tracker::EntityTracker;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Debug)]
pub enum PositionTracker {
    Entity(EntityTracker),
    BlockPos(BlockPosTracker),
}

impl PositionTracker {
    pub fn current_position(&self, level: &ServerLevel) -> Vec3 {
        match self {
            PositionTracker::Entity(tracker) => tracker.current_position(level),
            PositionTracker::BlockPos(tracker) => tracker.current_position(),
        }
    }

    pub fn current_block_position(&self, level: &ServerLevel) -> BlockPos {
        match self {
            PositionTracker::Entity(tracker) => tracker.current_block_position(level),
            PositionTracker::BlockPos(tracker) => tracker.current_block_position(),
        }
    }

    pub fn is_visible_by(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        match self {
            PositionTracker::Entity(tracker) => tracker.is_visible_by(level, body),
            PositionTracker::BlockPos(_) => true,
        }
    }
}

impl From<EntityTracker> for PositionTracker {
    fn from(tracker: EntityTracker) -> PositionTracker {
        PositionTracker::Entity(tracker)
    }
}

impl From<BlockPosTracker> for PositionTracker {
    fn from(tracker: BlockPosTracker) -> PositionTracker {
        PositionTracker::BlockPos(tracker)
    }
}
