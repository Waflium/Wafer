use crate::world::entity::data::entity_defaults;
use crate::world::entity::{Entity, EntityDimensions, EntityId};

pub struct EnderDragonPart {
    pub entity: Entity,
    pub parent_mob: EntityId,
    pub name: &'static str,
    pub size: EntityDimensions,
}

impl EnderDragonPart {
    pub fn new(parent_mob: EntityId, entity_type: i32, name: &'static str, w: f32, h: f32) -> EnderDragonPart {
        let size = EntityDimensions::scalable(w, h);
        EnderDragonPart { entity: Entity::new(entity_type, size, entity_defaults()), parent_mob, name, size }
    }
}
