use wafer_core::{BlockPos, Direction, mth};

use super::node::Node;
use super::path_type::PathType;
use super::pathfinding_context::PathfindingContext;
use super::target::Target;
use super::walk_node_evaluator::WalkNodeEvaluator;
use crate::world::entity::EntityId;

pub struct AmphibiousNodeEvaluator {
    prefers_shallow_swimming: bool,
    old_walkable_cost: f32,
    old_water_border_cost: f32,
}

impl AmphibiousNodeEvaluator {
    pub fn new(prefers_shallow_swimming: bool) -> AmphibiousNodeEvaluator {
        AmphibiousNodeEvaluator { prefers_shallow_swimming, old_walkable_cost: 0.0, old_water_border_cost: 0.0 }
    }
}

impl WalkNodeEvaluator {
    fn amphibious_mut(&mut self) -> &mut AmphibiousNodeEvaluator {
        self.amphibious.as_mut().expect("an amphibious node evaluator")
    }

    pub fn amphibious_node_evaluator_prepare(&mut self, context: &PathfindingContext, entity: EntityId) {
        self.walk_node_evaluator_prepare(context, entity);
        let mob = self.mob(context);
        mob.set_pathfinding_malus(PathType::Water, 0.0);
        let old_walkable_cost = mob.get_pathfinding_malus(PathType::Walkable);
        mob.set_pathfinding_malus(PathType::Walkable, 6.0);
        let old_water_border_cost = mob.get_pathfinding_malus(PathType::WaterBorder);
        mob.set_pathfinding_malus(PathType::WaterBorder, 4.0);
        let amphibious = self.amphibious_mut();
        amphibious.old_walkable_cost = old_walkable_cost;
        amphibious.old_water_border_cost = old_water_border_cost;
    }

    pub fn amphibious_node_evaluator_done(&mut self, context: &PathfindingContext) {
        let (old_walkable_cost, old_water_border_cost) = {
            let amphibious = self.amphibious_mut();
            (amphibious.old_walkable_cost, amphibious.old_water_border_cost)
        };
        let mob = self.mob(context);
        mob.set_pathfinding_malus(PathType::Walkable, old_walkable_cost);
        mob.set_pathfinding_malus(PathType::WaterBorder, old_water_border_cost);
        self.walk_node_evaluator_done(context);
    }

    pub fn amphibious_node_evaluator_get_start(&mut self, context: &PathfindingContext) -> Option<u32> {
        let e = &self.mob(context).entity;
        if !e.is_in_water() {
            return self.walk_node_evaluator_get_start(context);
        }
        let bb = e.bounding_box();
        let pos = BlockPos::new(mth::floor(bb.min_x), mth::floor(bb.min_y + 0.5), mth::floor(bb.min_z));
        Some(self.get_start_node(context, pos))
    }

    pub fn amphibious_node_evaluator_get_target(&mut self, x: f64, y: f64, z: f64) -> Target {
        self.node_evaluator.get_target_node_at(x, y + 0.5, z)
    }

    pub fn amphibious_node_evaluator_get_neighbors(&mut self, context: &PathfindingContext, neighbors: &mut [u32; 32], pos: u32) -> usize {
        let mut num_valid_neighbors = self.walk_node_evaluator_get_neighbors(context, neighbors, pos);
        let Node { x, y, z, .. } = self.node_evaluator.nodes[pos as usize];
        let block_path_type_above = self.get_cached_path_type(context, x, y + 1, z);
        let block_path_type_current = self.get_cached_path_type(context, x, y, z);
        let max_up_step = context.level().level.max_up_step(self.node_evaluator.mob);
        let jump_size = if self.mob(context).get_pathfinding_malus(block_path_type_above) >= 0.0 && block_path_type_current != PathType::StickyHoney {
            mth::floor_f(1.0f32.max(max_up_step))
        } else {
            0
        };
        let pos_height = self.get_floor_level(context, BlockPos::new(x, y, z));
        let up_node = self.find_accepted_node(context, x, y + 1, z, 0.max(jump_size - 1), pos_height, Direction::Up, block_path_type_current);
        let down_node = self.find_accepted_node(context, x, y - 1, z, jump_size, pos_height, Direction::Down, block_path_type_current);
        if self.is_vertical_neighbor_valid(up_node, pos) {
            neighbors[num_valid_neighbors] = up_node.expect("a valid neighbor");
            num_valid_neighbors += 1;
        }
        if self.is_vertical_neighbor_valid(down_node, pos) && block_path_type_current != PathType::Trapdoor {
            neighbors[num_valid_neighbors] = down_node.expect("a valid neighbor");
            num_valid_neighbors += 1;
        }
        let prefers_shallow_swimming = self.amphibious_mut().prefers_shallow_swimming;
        let sea_level = context.level().level.sea_level();
        for &neighbor in &neighbors[..num_valid_neighbors] {
            let neighbor = &mut self.node_evaluator.nodes[neighbor as usize];
            if neighbor.r#type == PathType::Water && prefers_shallow_swimming && neighbor.y < sea_level - 10 {
                neighbor.cost_malus += 1.0;
            }
        }
        num_valid_neighbors
    }

    fn is_vertical_neighbor_valid(&self, vertical_node: Option<u32>, pos: u32) -> bool {
        self.is_neighbor_valid(vertical_node, pos) && vertical_node.is_some_and(|node| self.node_evaluator.nodes[node as usize].r#type == PathType::Water)
    }

    pub fn amphibious_node_evaluator_get_path_type(&self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
        let block_path_type = context.get_path_type_from_state(x, y, z);
        if block_path_type == PathType::Water {
            for direction in Direction::VALUES {
                let reusable_pos = BlockPos::new(x, y, z).relative(direction);
                let path_type = context.get_path_type_from_state(reusable_pos.x, reusable_pos.y, reusable_pos.z);
                if path_type == PathType::Blocked {
                    return PathType::WaterBorder;
                }
            }
            PathType::Water
        } else {
            self.walk_node_evaluator_get_path_type(context, x, y, z)
        }
    }
}
