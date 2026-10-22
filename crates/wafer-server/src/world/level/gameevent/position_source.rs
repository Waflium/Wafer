use wafer_core::{BlockPos, Vec3};
use wafer_data::{registry, registry_id};
use wafer_protocol::Writer;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum PositionSource {
    Block(BlockPos),
    Entity { entity: EntityId, y_offset: f32 },
}

impl PositionSource {
    pub fn get_position(self, level: &ServerLevel) -> Option<Vec3> {
        match self {
            PositionSource::Block(pos) => Some(Vec3::at_center_of(pos)),
            PositionSource::Entity { entity, y_offset } => level.entity_ref(entity).filter(|e| !e.is_removed()).map(|e| e.position().add(0.0, y_offset as f64, 0.0)),
        }
    }

    pub fn write(self, w: &mut Writer) {
        match self {
            PositionSource::Block(pos) => {
                w.varint(registry_id(registry::POSITION_SOURCE_TYPE, "minecraft:block"));
                w.i64(pos.as_long());
            }
            PositionSource::Entity { entity, y_offset } => {
                w.varint(registry_id(registry::POSITION_SOURCE_TYPE, "minecraft:entity"));
                w.varint(entity);
                w.f32(y_offset);
            }
        }
    }
}
