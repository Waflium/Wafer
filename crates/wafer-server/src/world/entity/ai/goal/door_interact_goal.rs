use wafer_core::BlockPos;
use wafer_data::properties::OPEN;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::BlockGetter;
use crate::world::level::block::door_block;

pub struct DoorInteractGoal {
    pub door_pos: BlockPos,
    pub has_door: bool,
    passed: bool,
    door_open_dir_x: f32,
    door_open_dir_z: f32,
}

impl Default for DoorInteractGoal {
    fn default() -> DoorInteractGoal {
        DoorInteractGoal { door_pos: BlockPos::ZERO, has_door: false, passed: false, door_open_dir_x: 0.0, door_open_dir_z: 0.0 }
    }
}

impl DoorInteractGoal {
    pub fn is_open(&mut self, level: &mut ServerLevel) -> bool {
        if !self.has_door {
            return false;
        }
        let block_state = level.block_state(self.door_pos);
        if !block_state.block().is_instance_of("DoorBlock") {
            self.has_door = false;
            return false;
        }
        block_state.get(OPEN)
    }

    pub fn set_open(&self, level: &mut ServerLevel, mob: EntityId, open: bool) {
        if self.has_door {
            let block_state = level.block_state(self.door_pos);
            if block_state.block().is_instance_of("DoorBlock") {
                door_block::set_open(Some(mob), level, block_state, self.door_pos, open);
            }
        }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if !level.entity(mob).horizontal_collision {
            return false;
        }
        let m = level.mob(mob).expect("a mob");
        let Some(path) = m.ai.navigation.get_path().filter(|path| !path.is_done()) else { return false };
        let nodes: Vec<BlockPos> = (0..(path.get_next_node_index() as usize + 2).min(path.get_node_count())).map(|i| path.get_node_pos(i)).collect();
        for node in nodes {
            self.door_pos = BlockPos::new(node.x, node.y + 1, node.z);
            let e = level.entity(mob);
            if !(e.distance_to_sqr_xyz(self.door_pos.x as f64, e.y(), self.door_pos.z as f64) > 2.25) {
                self.has_door = door_block::is_wooden_door(level, self.door_pos);
                if self.has_door {
                    return true;
                }
            }
        }
        self.door_pos = level.entity(mob).block_position().above();
        self.has_door = door_block::is_wooden_door(level, self.door_pos);
        self.has_door
    }

    pub fn can_continue_to_use(&self) -> bool {
        !self.passed
    }

    pub fn start(&mut self, level: &ServerLevel, mob: EntityId) {
        self.passed = false;
        let e = level.entity(mob);
        self.door_open_dir_x = (self.door_pos.x as f64 + 0.5 - e.x()) as f32;
        self.door_open_dir_z = (self.door_pos.z as f64 + 0.5 - e.z()) as f32;
    }

    pub fn tick(&mut self, level: &ServerLevel, mob: EntityId) {
        let e = level.entity(mob);
        let new_door_dir_x = (self.door_pos.x as f64 + 0.5 - e.x()) as f32;
        let new_door_dir_z = (self.door_pos.z as f64 + 0.5 - e.z()) as f32;
        let dot = self.door_open_dir_x * new_door_dir_x + self.door_open_dir_z * new_door_dir_z;
        if dot < 0.0 {
            self.passed = true;
        }
    }
}
