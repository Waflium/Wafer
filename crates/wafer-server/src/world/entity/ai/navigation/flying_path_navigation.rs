use wafer_core::{BlockPos, Direction, mth};

use super::path_navigation::PathNavigation;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::node_evaluator::NodeEvaluatorKind;
use crate::world::level::pathfinder::path_finder::PathFinder;
use crate::world::level::pathfinder::walk_node_evaluator::WalkNodeEvaluator;
use crate::world::phys::shapes::collision_context::CollisionContext;

impl PathNavigation {
    pub fn flying_path_navigation_create_path_finder(&mut self, max_visited_nodes: i32) -> PathFinder {
        PathFinder::new(NodeEvaluatorKind::Walk(WalkNodeEvaluator::fly()), max_visited_nodes)
    }

    pub fn flying_path_navigation_can_update_path(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.can_float() && level.entity(mob).is_in_liquid() || !level.is_passenger(mob)
    }

    pub fn flying_path_navigation_tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.tick += 1;
        if self.has_delayed_recomputation {
            self.recompute_path(level, mob);
        }
        if self.is_done() {
            return;
        }
        if self.can_update_path(level, mob) {
            self.follow_the_path(level, mob);
        } else if let Some(path) = self.path.as_mut().filter(|path| !path.is_done()) {
            let e = level.entity(mob);
            let pos = path.get_next_entity_pos(e);
            let block_pos = e.block_position();
            if block_pos.x == mth::floor(pos.x) && block_pos.y == mth::floor(pos.y) && block_pos.z == mth::floor(pos.z) {
                path.advance();
            }
        }
        if !self.is_done() {
            let target = self.path.as_ref().expect("a path").get_next_entity_pos(level.entity(mob));
            level.set_wanted_position(mob, target.x, target.y, target.z, self.speed_modifier);
        }
    }

    pub fn flying_path_navigation_is_stable_destination(&self, level: &ServerLevel, mob: EntityId, pos: BlockPos) -> bool {
        let state = PathNavigationRegion::new(level).get_block_state(pos);
        let (shape, offset) = CollisionContext::of(level, mob).get_collision_shape(state, pos);
        wafer_core::shapes::is_face_full(&shape.moved(offset[0], offset[1], offset[2]), Direction::Up)
    }
}
