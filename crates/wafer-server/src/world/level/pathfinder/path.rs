use std::cell::RefCell;
use std::rc::Rc;

use wafer_core::{BlockPos, Vec3};

use super::node::Node;
use crate::world::entity::Entity;

#[derive(Debug)]
struct PathData {
    nodes: Vec<Node>,
    next_node_index: i32,
    target: BlockPos,
    dist_to_target: f32,
    reached: bool,
}

#[derive(Clone, Debug)]
pub struct Path(Rc<RefCell<PathData>>);

impl Path {
    pub fn new(nodes: Vec<Node>, target: BlockPos, reached: bool) -> Path {
        let dist_to_target = nodes.last().map_or(f32::MAX, |node| node.distance_manhattan_pos(target));
        Path(Rc::new(RefCell::new(PathData { nodes, next_node_index: 0, target, dist_to_target, reached })))
    }

    pub fn advance(&self) {
        self.0.borrow_mut().next_node_index += 1;
    }

    pub fn not_started(&self) -> bool {
        self.0.borrow().next_node_index <= 0
    }

    pub fn is_done(&self) -> bool {
        let data = self.0.borrow();
        data.next_node_index >= data.nodes.len() as i32
    }

    pub fn get_end_node(&self) -> Option<Node> {
        self.0.borrow().nodes.last().copied()
    }

    pub fn get_node(&self, i: usize) -> Node {
        self.0.borrow().nodes[i]
    }

    pub fn truncate_nodes(&self, index: usize) {
        self.0.borrow_mut().nodes.truncate(index);
    }

    pub fn replace_node(&self, index: usize, replace_with: Node) {
        self.0.borrow_mut().nodes[index] = replace_with;
    }

    pub fn get_node_count(&self) -> usize {
        self.0.borrow().nodes.len()
    }

    pub fn get_next_node_index(&self) -> i32 {
        self.0.borrow().next_node_index
    }

    pub fn set_next_node_index(&self, next_node_index: i32) {
        self.0.borrow_mut().next_node_index = next_node_index;
    }

    pub fn get_entity_pos_at_node(&self, entity: &Entity, index: usize) -> Vec3 {
        let node = self.0.borrow().nodes[index];
        let offset = (entity.dimensions.width + 1.0) as i32 as f64 * 0.5;
        Vec3::new(node.x as f64 + offset, node.y as f64, node.z as f64 + offset)
    }

    pub fn get_node_pos(&self, index: usize) -> BlockPos {
        self.0.borrow().nodes[index].as_block_pos()
    }

    pub fn get_next_entity_pos(&self, entity: &Entity) -> Vec3 {
        self.get_entity_pos_at_node(entity, self.get_next_node_index() as usize)
    }

    pub fn get_next_node_pos(&self) -> BlockPos {
        let data = self.0.borrow();
        data.nodes[data.next_node_index as usize].as_block_pos()
    }

    pub fn get_next_node(&self) -> Node {
        let data = self.0.borrow();
        data.nodes[data.next_node_index as usize]
    }

    pub fn get_previous_node(&self) -> Option<Node> {
        let data = self.0.borrow();
        (data.next_node_index > 0).then(|| data.nodes[data.next_node_index as usize - 1])
    }

    pub fn same_as(&self, path: Option<&Path>) -> bool {
        path.is_some_and(|path| Rc::ptr_eq(&self.0, &path.0) || self.0.borrow().nodes == path.0.borrow().nodes)
    }

    pub fn can_reach(&self) -> bool {
        self.0.borrow().reached
    }

    pub fn get_target(&self) -> BlockPos {
        self.0.borrow().target
    }

    pub fn get_dist_to_target(&self) -> f32 {
        self.0.borrow().dist_to_target
    }
}
