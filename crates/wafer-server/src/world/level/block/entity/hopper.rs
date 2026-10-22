use wafer_core::{AABB, BlockPos};

use super::hopper_block_entity::SUCK_AABB;
use crate::server::level::ServerLevel;
use crate::world::container::ContainerRef;
use crate::world::entity::EntityId;

#[derive(Clone, Copy)]
pub enum Hopper {
    Block(BlockPos),
    Minecart(EntityId),
}

impl Hopper {
    pub fn get_suck_aabb(self) -> AABB {
        SUCK_AABB
    }

    pub fn get_level_x(self, level: &ServerLevel) -> f64 {
        match self {
            Hopper::Block(pos) => pos.x as f64 + 0.5,
            Hopper::Minecart(id) => level.entity(id).x(),
        }
    }

    pub fn get_level_y(self, level: &ServerLevel) -> f64 {
        match self {
            Hopper::Block(pos) => pos.y as f64 + 0.5,
            Hopper::Minecart(id) => level.entity(id).y() + 0.5,
        }
    }

    pub fn get_level_z(self, level: &ServerLevel) -> f64 {
        match self {
            Hopper::Block(pos) => pos.z as f64 + 0.5,
            Hopper::Minecart(id) => level.entity(id).z(),
        }
    }

    pub fn is_grid_aligned(self) -> bool {
        matches!(self, Hopper::Block(_))
    }

    pub fn container(self, level: &mut ServerLevel) -> ContainerRef {
        match self {
            Hopper::Block(pos) => ContainerRef::block(level, pos),
            Hopper::Minecart(id) => ContainerRef::ChestVehicle(id),
        }
    }
}
