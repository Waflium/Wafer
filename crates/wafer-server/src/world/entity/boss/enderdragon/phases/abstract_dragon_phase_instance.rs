use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub const DEFAULT_FLY_SPEED: f32 = 0.6;

impl ServerLevel {
    pub fn abstract_dragon_phase_get_turn_speed(&self, dragon: EntityId) -> f32 {
        let rot_speed = self.entity(dragon).delta_movement.horizontal_distance() as f32 + 1.0;
        let dist = rot_speed.min(40.0);
        0.7 / dist / rot_speed
    }
}
