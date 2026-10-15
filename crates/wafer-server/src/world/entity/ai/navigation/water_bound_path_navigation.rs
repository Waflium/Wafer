use wafer_core::{BlockPos, Vec3};

use super::path_navigation::{PathNavigation, is_clear_for_movement_between};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::path_navigation_region::PathNavigationRegion;
use crate::world::level::pathfinder::node_evaluator::NodeEvaluatorKind;
use crate::world::level::pathfinder::path_finder::PathFinder;
use crate::world::level::pathfinder::swim_node_evaluator::SwimNodeEvaluator;

impl PathNavigation {
    pub fn water_bound_path_navigation_create_path_finder(&mut self, max_visited_nodes: i32) -> PathFinder {
        let mut node_evaluator = SwimNodeEvaluator::new(self.allow_breaching);
        node_evaluator.node_evaluator.can_pass_doors = false;
        PathFinder::new(NodeEvaluatorKind::Swim(node_evaluator), max_visited_nodes)
    }

    pub fn water_bound_path_navigation_can_update_path(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.allow_breaching || level.entity(mob).is_in_liquid()
    }

    pub fn water_bound_path_navigation_get_temp_mob_pos(&self, level: &ServerLevel, mob: EntityId) -> Vec3 {
        let e = level.entity(mob);
        Vec3::new(e.x(), e.y() + e.dimensions.height as f64 * 0.5, e.z())
    }

    pub fn water_bound_path_navigation_get_ground_y(&self, target: Vec3) -> f64 {
        target.y
    }

    pub fn water_bound_path_navigation_can_move_directly(&self, level: &mut ServerLevel, mob: EntityId, start_pos: Vec3, stop_pos: Vec3) -> bool {
        is_clear_for_movement_between(level, mob, start_pos, stop_pos, false)
    }

    pub fn water_bound_path_navigation_is_stable_destination(&self, level: &ServerLevel, pos: BlockPos) -> bool {
        !PathNavigationRegion::new(level).get_block_state(pos).is_solid_render()
    }

    pub fn water_bound_path_navigation_set_can_float(&mut self, _can_float: bool) {}

    pub fn water_bound_path_navigation_can_navigate_ground(&self) -> bool {
        false
    }

    pub fn water_bound_path_navigation_get_max_vertical_distance_to_waypoint(&self) -> f32 {
        0.5
    }
}
