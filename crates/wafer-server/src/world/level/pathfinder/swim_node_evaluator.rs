use wafer_core::hash_map::FastMap;
use wafer_core::{BlockPos, Direction, mth};
use wafer_data::fluid_tags;

use super::node::Node;
use super::node_evaluator::NodeEvaluator;
use super::path_type::PathType;
use super::pathfinding_context::PathfindingContext;
use super::target::Target;
use crate::world::entity::EntityId;
use crate::world::entity::mob::Mob;

pub struct SwimNodeEvaluator {
    pub node_evaluator: NodeEvaluator,
    allow_breaching: bool,
    path_types_by_pos_cache: FastMap<i64, PathType>,
}

impl SwimNodeEvaluator {
    pub fn new(allow_breaching: bool) -> SwimNodeEvaluator {
        SwimNodeEvaluator { node_evaluator: NodeEvaluator::default(), allow_breaching, path_types_by_pos_cache: FastMap::default() }
    }

    fn mob<'b>(&self, context: &'b PathfindingContext) -> &'b Mob {
        context.level().level.mob(self.node_evaluator.mob).expect("a pathfinding mob")
    }

    pub fn prepare(&mut self, context: &PathfindingContext, entity: EntityId) {
        self.node_evaluator.prepare(context, entity);
        self.path_types_by_pos_cache.clear();
    }

    pub fn done(&mut self) {
        self.path_types_by_pos_cache.clear();
    }

    pub fn get_start(&mut self, context: &PathfindingContext) -> Option<u32> {
        let bb = self.mob(context).entity.bounding_box();
        Some(self.node_evaluator.get_node(mth::floor(bb.min_x), mth::floor(bb.min_y + 0.5), mth::floor(bb.min_z)))
    }

    pub fn get_target(&mut self, x: f64, y: f64, z: f64) -> Target {
        self.node_evaluator.get_target_node_at(x, y, z)
    }

    pub fn get_neighbors(&mut self, context: &PathfindingContext, neighbors: &mut [u32; 32], pos: u32) -> usize {
        let mut count = 0;
        let mut nodes: [Option<u32>; 6] = [None; 6];
        let Node { x, y, z, .. } = self.node_evaluator.nodes[pos as usize];
        for direction in Direction::VALUES {
            let node = self.find_accepted_node(context, x + direction.step_x(), y + direction.step_y(), z + direction.step_z());
            nodes[direction as usize] = node;
            if self.is_node_valid(node) {
                neighbors[count] = node.expect("a valid neighbor");
                count += 1;
            }
        }
        for direction in Direction::PLANE_HORIZONTAL {
            let second_direction = direction.clockwise();
            if self.has_malus(nodes[direction as usize]) && self.has_malus(nodes[second_direction as usize]) {
                let diagonal_node = self.find_accepted_node(
                    context,
                    x + direction.step_x() + second_direction.step_x(),
                    y,
                    z + direction.step_z() + second_direction.step_z(),
                );
                if self.is_node_valid(diagonal_node) {
                    neighbors[count] = diagonal_node.expect("a valid diagonal");
                    count += 1;
                }
            }
        }
        count
    }

    fn is_node_valid(&self, node: Option<u32>) -> bool {
        node.is_some_and(|node| !self.node_evaluator.nodes[node as usize].closed)
    }

    fn has_malus(&self, node: Option<u32>) -> bool {
        node.is_some_and(|node| self.node_evaluator.nodes[node as usize].cost_malus >= 0.0)
    }

    fn find_accepted_node(&mut self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> Option<u32> {
        let path_type = self.get_cached_block_type(context, x, y, z);
        if !(self.allow_breaching && path_type == PathType::Breach || path_type == PathType::Water) {
            return None;
        }
        let path_cost = self.mob(context).get_pathfinding_malus(path_type);
        if path_cost < 0.0 {
            return None;
        }
        let best = self.node_evaluator.get_node(x, y, z);
        let fluid_is_empty = context.level().get_fluid_state(BlockPos::new(x, y, z)).is_empty();
        let node = &mut self.node_evaluator.nodes[best as usize];
        node.r#type = path_type;
        node.cost_malus = node.cost_malus.max(path_cost);
        if fluid_is_empty {
            node.cost_malus += 8.0;
        }
        Some(best)
    }

    fn get_cached_block_type(&mut self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
        let key = BlockPos::new(x, y, z).as_long();
        if let Some(&path_type) = self.path_types_by_pos_cache.get(&key) {
            return path_type;
        }
        let path_type = self.get_path_type(context, x, y, z);
        self.path_types_by_pos_cache.insert(key, path_type);
        path_type
    }

    pub fn get_path_type(&self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
        self.get_path_type_of_mob(context, x, y, z)
    }

    pub fn get_path_type_of_mob(&self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
        let evaluator = &self.node_evaluator;
        let mut pos = BlockPos::new(x, y, z);
        for xx in x..x + evaluator.entity_width {
            for yy in y..y + evaluator.entity_height {
                for zz in z..z + evaluator.entity_depth {
                    pos = BlockPos::new(xx, yy, zz);
                    let block_state = context.get_block_state(pos);
                    let fluid_state = block_state.fluid_state();
                    let below_state = context.get_block_state(pos.below());
                    if fluid_state.is_empty() && below_state.is_pathfindable_water() && block_state.is_air() {
                        return PathType::Breach;
                    }
                    if !fluid_state.is(&fluid_tags::WATER) {
                        return PathType::Blocked;
                    }
                }
            }
        }
        if context.get_block_state(pos).is_pathfindable_water() { PathType::Water } else { PathType::Blocked }
    }
}
