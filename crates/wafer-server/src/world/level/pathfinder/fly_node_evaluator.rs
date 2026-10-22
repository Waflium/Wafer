use wafer_core::random::{LegacyRandomSource, RandomSource};
use wafer_core::{BlockPos, mth};
use wafer_data::blocks;

use super::path_type::PathType;
use super::pathfinding_context::PathfindingContext;
use super::target::Target;
use super::walk_node_evaluator::{WalkNodeEvaluator, check_neighbour_blocks};

const SMALL_MOB_SIZE: f32 = 1.0;
const SMALL_MOB_INFLATED_START_NODE_BOUNDING_BOX: f32 = 1.1;
const MAX_START_NODE_CANDIDATES: i32 = 10;

enum StartNodeCandidates {
    Corners([BlockPos; 4]),
    RandomBetweenClosed(BlockPos, BlockPos),
}

pub struct FlyNodeEvaluator {
    pub random: LegacyRandomSource,
}

impl Default for FlyNodeEvaluator {
    fn default() -> FlyNodeEvaluator {
        FlyNodeEvaluator { random: LegacyRandomSource::new(0) }
    }
}

impl WalkNodeEvaluator {
    pub fn fly_node_evaluator_get_start(&mut self, context: &PathfindingContext) -> Option<u32> {
        let e = &self.mob(context).entity;
        let (x, y, z) = (e.x(), e.y(), e.z());
        let start_y = if self.node_evaluator.can_float && e.is_in_water() {
            let mut start_y = e.block_position().y;
            while context.get_block_state(BlockPos::containing(x, start_y as f64, z)).is_block(blocks::WATER) {
                start_y += 1;
            }
            start_y
        } else {
            mth::floor(y + 0.5)
        };
        let start_pos = BlockPos::containing(x, start_y as f64, z);
        if !self.fly_node_evaluator_can_start_at(context, start_pos) {
            match self.iterate_pathfinding_start_node_candidate_positions(context) {
                StartNodeCandidates::Corners(corners) => {
                    for tested_position in corners {
                        if self.fly_node_evaluator_can_start_at(context, tested_position) {
                            return Some(self.get_start_node(context, tested_position));
                        }
                    }
                }
                StartNodeCandidates::RandomBetweenClosed(min, max) => {
                    let (width, height, depth) = (max.x - min.x + 1, max.y - min.y + 1, max.z - min.z + 1);
                    for _ in 0..MAX_START_NODE_CANDIDATES {
                        let random = &mut self.fly.as_mut().expect("a fly node evaluator").random;
                        let x = min.x + random.next_int_bounded(width);
                        let y = min.y + random.next_int_bounded(height);
                        let z = min.z + random.next_int_bounded(depth);
                        let tested_position = BlockPos::new(x, y, z);
                        if self.fly_node_evaluator_can_start_at(context, tested_position) {
                            return Some(self.get_start_node(context, tested_position));
                        }
                    }
                }
            }
        }
        Some(self.get_start_node(context, start_pos))
    }

    fn fly_node_evaluator_can_start_at(&mut self, context: &PathfindingContext, pos: BlockPos) -> bool {
        let block_path_type = self.get_cached_path_type(context, pos.x, pos.y, pos.z);
        self.mob(context).get_pathfinding_malus(block_path_type) >= 0.0
    }

    pub fn fly_node_evaluator_get_target(&mut self, x: f64, y: f64, z: f64) -> Target {
        self.node_evaluator.get_target_node_at(x, y, z)
    }

    pub fn fly_node_evaluator_get_neighbors(&mut self, context: &PathfindingContext, neighbors: &mut [u32; 32], pos: u32) -> usize {
        let node = &self.node_evaluator.nodes[pos as usize];
        let (x, y, z) = (node.x, node.y, node.z);
        let mut count = 0;
        let mut push = |node: u32, count: &mut usize| {
            neighbors[*count] = node;
            *count += 1;
        };
        let south = self.find_accepted_fly_node(context, x, y, z + 1);
        if self.is_open(south) {
            push(south.unwrap(), &mut count);
        }
        let west = self.find_accepted_fly_node(context, x - 1, y, z);
        if self.is_open(west) {
            push(west.unwrap(), &mut count);
        }
        let east = self.find_accepted_fly_node(context, x + 1, y, z);
        if self.is_open(east) {
            push(east.unwrap(), &mut count);
        }
        let north = self.find_accepted_fly_node(context, x, y, z - 1);
        if self.is_open(north) {
            push(north.unwrap(), &mut count);
        }
        let up = self.find_accepted_fly_node(context, x, y + 1, z);
        if self.is_open(up) {
            push(up.unwrap(), &mut count);
        }
        let down = self.find_accepted_fly_node(context, x, y - 1, z);
        if self.is_open(down) {
            push(down.unwrap(), &mut count);
        }
        let south_up = self.find_accepted_fly_node(context, x, y + 1, z + 1);
        if self.is_open(south_up) && self.has_malus(south) && self.has_malus(up) {
            push(south_up.unwrap(), &mut count);
        }
        let west_up = self.find_accepted_fly_node(context, x - 1, y + 1, z);
        if self.is_open(west_up) && self.has_malus(west) && self.has_malus(up) {
            push(west_up.unwrap(), &mut count);
        }
        let east_up = self.find_accepted_fly_node(context, x + 1, y + 1, z);
        if self.is_open(east_up) && self.has_malus(east) && self.has_malus(up) {
            push(east_up.unwrap(), &mut count);
        }
        let north_up = self.find_accepted_fly_node(context, x, y + 1, z - 1);
        if self.is_open(north_up) && self.has_malus(north) && self.has_malus(up) {
            push(north_up.unwrap(), &mut count);
        }
        let south_down = self.find_accepted_fly_node(context, x, y - 1, z + 1);
        if self.is_open(south_down) && self.has_malus(south) && self.has_malus(down) {
            push(south_down.unwrap(), &mut count);
        }
        let west_down = self.find_accepted_fly_node(context, x - 1, y - 1, z);
        if self.is_open(west_down) && self.has_malus(west) && self.has_malus(down) {
            push(west_down.unwrap(), &mut count);
        }
        let east_down = self.find_accepted_fly_node(context, x + 1, y - 1, z);
        if self.is_open(east_down) && self.has_malus(east) && self.has_malus(down) {
            push(east_down.unwrap(), &mut count);
        }
        let north_down = self.find_accepted_fly_node(context, x, y - 1, z - 1);
        if self.is_open(north_down) && self.has_malus(north) && self.has_malus(down) {
            push(north_down.unwrap(), &mut count);
        }
        let north_east = self.find_accepted_fly_node(context, x + 1, y, z - 1);
        if self.is_open(north_east) && self.has_malus(north) && self.has_malus(east) {
            push(north_east.unwrap(), &mut count);
        }
        let south_east = self.find_accepted_fly_node(context, x + 1, y, z + 1);
        if self.is_open(south_east) && self.has_malus(south) && self.has_malus(east) {
            push(south_east.unwrap(), &mut count);
        }
        let north_west = self.find_accepted_fly_node(context, x - 1, y, z - 1);
        if self.is_open(north_west) && self.has_malus(north) && self.has_malus(west) {
            push(north_west.unwrap(), &mut count);
        }
        let south_west = self.find_accepted_fly_node(context, x - 1, y, z + 1);
        if self.is_open(south_west) && self.has_malus(south) && self.has_malus(west) {
            push(south_west.unwrap(), &mut count);
        }
        let north_east_up = self.find_accepted_fly_node(context, x + 1, y + 1, z - 1);
        if self.is_open(north_east_up) && [north_east, north, east, up, north_up, east_up].into_iter().all(|node| self.has_malus(node)) {
            push(north_east_up.unwrap(), &mut count);
        }
        let south_east_up = self.find_accepted_fly_node(context, x + 1, y + 1, z + 1);
        if self.is_open(south_east_up) && [south_east, south, east, up, south_up, east_up].into_iter().all(|node| self.has_malus(node)) {
            push(south_east_up.unwrap(), &mut count);
        }
        let north_west_up = self.find_accepted_fly_node(context, x - 1, y + 1, z - 1);
        if self.is_open(north_west_up) && [north_west, north, west, up, north_up, west_up].into_iter().all(|node| self.has_malus(node)) {
            push(north_west_up.unwrap(), &mut count);
        }
        let south_west_up = self.find_accepted_fly_node(context, x - 1, y + 1, z + 1);
        if self.is_open(south_west_up) && [south_west, south, west, up, south_up, west_up].into_iter().all(|node| self.has_malus(node)) {
            push(south_west_up.unwrap(), &mut count);
        }
        let north_east_down = self.find_accepted_fly_node(context, x + 1, y - 1, z - 1);
        if self.is_open(north_east_down) && [north_east, north, east, down, north_down, east_down].into_iter().all(|node| self.has_malus(node)) {
            push(north_east_down.unwrap(), &mut count);
        }
        let south_east_down = self.find_accepted_fly_node(context, x + 1, y - 1, z + 1);
        if self.is_open(south_east_down) && [south_east, south, east, down, south_down, east_down].into_iter().all(|node| self.has_malus(node)) {
            push(south_east_down.unwrap(), &mut count);
        }
        let north_west_down = self.find_accepted_fly_node(context, x - 1, y - 1, z - 1);
        if self.is_open(north_west_down) && [north_west, north, west, down, north_down, west_down].into_iter().all(|node| self.has_malus(node)) {
            push(north_west_down.unwrap(), &mut count);
        }
        let south_west_down = self.find_accepted_fly_node(context, x - 1, y - 1, z + 1);
        if self.is_open(south_west_down) && [south_west, south, west, down, south_down, west_down].into_iter().all(|node| self.has_malus(node)) {
            push(south_west_down.unwrap(), &mut count);
        }
        count
    }

    fn has_malus(&self, node: Option<u32>) -> bool {
        node.is_some_and(|node| self.node_evaluator.nodes[node as usize].cost_malus >= 0.0)
    }

    fn is_open(&self, node: Option<u32>) -> bool {
        node.is_some_and(|node| !self.node_evaluator.nodes[node as usize].closed)
    }

    fn find_accepted_fly_node(&mut self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> Option<u32> {
        let path_type = self.get_cached_path_type(context, x, y, z);
        let path_cost = self.mob(context).get_pathfinding_malus(path_type);
        if path_cost < 0.0 {
            return None;
        }
        let best = self.node_evaluator.get_node(x, y, z);
        let node = &mut self.node_evaluator.nodes[best as usize];
        node.r#type = path_type;
        node.cost_malus = node.cost_malus.max(path_cost);
        if path_type == PathType::Walkable {
            node.cost_malus += 1.0;
        }
        Some(best)
    }

    pub fn fly_node_evaluator_get_path_type(&self, context: &PathfindingContext, x: i32, y: i32, z: i32) -> PathType {
        let mut block_path_type = context.get_path_type_from_state(x, y, z);
        if block_path_type == PathType::Open && y >= context.level().get_min_y() + 1 {
            let below_pos = BlockPos::new(x, y - 1, z);
            let below_type = context.get_path_type_from_state(below_pos.x, below_pos.y, below_pos.z);
            block_path_type = if below_type == PathType::Fire || below_type == PathType::Lava {
                PathType::Fire
            } else if below_type == PathType::Damaging {
                PathType::Damaging
            } else if below_type == PathType::Cocoa {
                PathType::Cocoa
            } else if below_type == PathType::Fence {
                if below_pos != context.mob_position() { PathType::Fence } else { block_path_type }
            } else if below_type != PathType::Walkable && below_type != PathType::Open && below_type != PathType::Water {
                PathType::Walkable
            } else {
                PathType::Open
            };
        }
        if block_path_type == PathType::Walkable || block_path_type == PathType::Open {
            block_path_type = check_neighbour_blocks(context, x, y, z, block_path_type);
        }
        block_path_type
    }

    fn iterate_pathfinding_start_node_candidate_positions(&self, context: &PathfindingContext) -> StartNodeCandidates {
        let e = &self.mob(context).entity;
        let bounding_box = e.bounding_box();
        let block_y = e.block_position().y;
        let is_small_mob = bounding_box.size() < SMALL_MOB_SIZE as f64;
        if !is_small_mob {
            return StartNodeCandidates::Corners([
                BlockPos::containing(bounding_box.min_x, block_y as f64, bounding_box.min_z),
                BlockPos::containing(bounding_box.min_x, block_y as f64, bounding_box.max_z),
                BlockPos::containing(bounding_box.max_x, block_y as f64, bounding_box.min_z),
                BlockPos::containing(bounding_box.max_x, block_y as f64, bounding_box.max_z),
            ]);
        }
        let z_padding = 0.0f64.max(SMALL_MOB_INFLATED_START_NODE_BOUNDING_BOX as f64 - bounding_box.z_size());
        let x_padding = 0.0f64.max(SMALL_MOB_INFLATED_START_NODE_BOUNDING_BOX as f64 - bounding_box.x_size());
        let y_padding = 0.0f64.max(SMALL_MOB_INFLATED_START_NODE_BOUNDING_BOX as f64 - bounding_box.y_size());
        let inflated = bounding_box.inflate(x_padding, y_padding, z_padding);
        StartNodeCandidates::RandomBetweenClosed(
            BlockPos::new(mth::floor(inflated.min_x), mth::floor(inflated.min_y), mth::floor(inflated.min_z)),
            BlockPos::new(mth::floor(inflated.max_x), mth::floor(inflated.max_y), mth::floor(inflated.max_z)),
        )
    }
}

