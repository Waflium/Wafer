use wafer_core::BlockPos;
use wafer_data::BlockState;

use super::path_type::PathType;
use super::walk_node_evaluator::get_path_type_from_state;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::path_navigation_region::PathNavigationRegion;

pub trait PathTypeGetter {
    fn get_path_type_from_state(&mut self, x: i32, y: i32, z: i32) -> PathType;

    fn get_min_y(&self) -> i32;
}

pub struct PathfindingContext<'a> {
    level: PathNavigationRegion<'a>,
    mob_position: BlockPos,
}

impl<'a> PathfindingContext<'a> {
    pub fn new(level: PathNavigationRegion<'a>, mob: EntityId) -> PathfindingContext<'a> {
        let mob_position = level.level.entity(mob).block_position();
        PathfindingContext { level, mob_position }
    }

    pub fn get_path_type_from_state(&self, x: i32, y: i32, z: i32) -> PathType {
        let pos = BlockPos::new(x, y, z);
        let mut level = self.level;
        self.level.level.path_types_by_pos_cache.borrow_mut().get_or_compute(&mut level, pos)
    }

    pub fn get_block_state(&self, pos: BlockPos) -> BlockState {
        self.level.get_block_state(pos)
    }

    pub fn level(&self) -> &PathNavigationRegion<'a> {
        &self.level
    }

    pub fn mob_position(&self) -> BlockPos {
        self.mob_position
    }
}

impl PathTypeGetter for &PathfindingContext<'_> {
    fn get_path_type_from_state(&mut self, x: i32, y: i32, z: i32) -> PathType {
        PathfindingContext::get_path_type_from_state(self, x, y, z)
    }

    fn get_min_y(&self) -> i32 {
        self.level.get_min_y()
    }
}

pub struct LevelPathfindingContext<'a> {
    level: &'a mut ServerLevel,
}

impl<'a> LevelPathfindingContext<'a> {
    pub fn new(level: &'a mut ServerLevel) -> LevelPathfindingContext<'a> {
        LevelPathfindingContext { level }
    }
}

impl PathTypeGetter for LevelPathfindingContext<'_> {
    fn get_path_type_from_state(&mut self, x: i32, y: i32, z: i32) -> PathType {
        let pos = BlockPos::new(x, y, z);
        let cached = self.level.path_types_by_pos_cache.borrow().get_cached(pos);
        cached.unwrap_or_else(|| {
            let path_type = get_path_type_from_state(self.level, pos);
            self.level.path_types_by_pos_cache.borrow_mut().store(pos, path_type);
            path_type
        })
    }

    fn get_min_y(&self) -> i32 {
        self.level.chunks.min_y()
    }
}
