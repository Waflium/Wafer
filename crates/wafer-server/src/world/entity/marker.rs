use wafer_data::{registry, registry_id};

use super::data::entity_defaults;
use super::{Entity, EntityDimensions};

pub const DIMENSIONS: EntityDimensions = EntityDimensions::scalable(0.0, 0.0);

pub struct Marker {
    pub entity: Entity,
}

impl Marker {
    pub fn create() -> Marker {
        let mut entity = Entity::new(registry_id(registry::ENTITY_TYPE, "minecraft:marker"), DIMENSIONS, entity_defaults());
        entity.no_physics = true;
        Marker { entity }
    }

    pub fn can_add_passenger() -> bool {
        false
    }

    pub fn could_accept_passenger() -> bool {
        false
    }

    pub fn is_ignoring_block_triggers() -> bool {
        true
    }

    pub fn hurt_server() -> bool {
        false
    }
}
