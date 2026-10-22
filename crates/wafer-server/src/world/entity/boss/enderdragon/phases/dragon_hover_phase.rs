use wafer_core::Vec3;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub const FLY_SPEED: f32 = 1.0;

#[derive(Default)]
pub struct DragonHoverPhase {
    pub target_location: Option<Vec3>,
}

impl DragonHoverPhase {
    pub fn begin(&mut self) {
        self.target_location = None;
    }
}

impl ServerLevel {
    pub fn dragon_hover_do_server_tick(&mut self, dragon: EntityId) {
        if self.dragon_phase_manager(dragon).hovering.target_location.is_none() {
            let position = self.entity(dragon).position();
            self.dragon_phase_manager_mut(dragon).hovering.target_location = Some(position);
        }
    }
}
