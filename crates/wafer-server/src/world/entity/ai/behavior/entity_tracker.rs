use wafer_core::{BlockPos, Vec3};

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::memory::memory_module_type;

#[derive(Clone, Debug)]
pub struct EntityTracker {
    entity: EntityId,
    track_eye_height: bool,
    target_eye_height: bool,
    position: Vec3,
    eye_height: f64,
}

impl EntityTracker {
    pub fn new(level: &ServerLevel, entity: EntityId, track_eye_height: bool) -> EntityTracker {
        EntityTracker::with_target_eye_height(level, entity, track_eye_height, false)
    }

    pub fn with_target_eye_height(level: &ServerLevel, entity: EntityId, track_eye_height: bool, target_eye_height: bool) -> EntityTracker {
        let e = level.entity(entity);
        EntityTracker { entity, track_eye_height, target_eye_height, position: e.position(), eye_height: e.dimensions.eye_height as f64 }
    }

    fn position_and_eye_height(&self, level: &ServerLevel) -> (Vec3, f64) {
        match level.entity_ref(self.entity) {
            Some(e) => (e.position(), e.dimensions.eye_height as f64),
            None => (self.position, self.eye_height),
        }
    }

    pub fn current_position(&self, level: &ServerLevel) -> Vec3 {
        let (position, eye_height) = self.position_and_eye_height(level);
        if self.track_eye_height { position.add(0.0, eye_height, 0.0) } else { position }
    }

    pub fn current_block_position(&self, level: &ServerLevel) -> BlockPos {
        let (position, eye_height) = self.position_and_eye_height(level);
        if self.target_eye_height { BlockPos::containing(position.x, position.y + eye_height, position.z) } else { BlockPos::containing_vec(position) }
    }

    pub fn is_visible_by(&self, level: &mut ServerLevel, body: EntityId) -> bool {
        if level.is_living(self.entity) || level.entity_ref(self.entity).is_none() {
            if !level.is_alive(self.entity) {
                return false;
            }
            let visible_entities = level.get_brain(body).get_memory(memory_module_type::NEAREST_VISIBLE_LIVING_ENTITIES).cloned();
            visible_entities.is_some_and(|visible_entities| visible_entities.contains(level, self.entity))
        } else {
            true
        }
    }

    pub fn get_entity(&self) -> EntityId {
        self.entity
    }
}
