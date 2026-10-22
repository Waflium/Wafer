use wafer_core::Vec3;

use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

const CHARGE_RECOVERY_TIME: i32 = 10;
pub const FLY_SPEED: f32 = 3.0;

#[derive(Default)]
pub struct DragonChargePlayerPhase {
    pub target_location: Option<Vec3>,
    pub time_since_charge: i32,
}

impl DragonChargePlayerPhase {
    pub fn begin(&mut self) {
        self.target_location = None;
        self.time_since_charge = 0;
    }
}

impl ServerLevel {
    pub fn dragon_charge_player_do_server_tick(&mut self, dragon: EntityId) {
        let phase = &mut self.dragon_phase_manager_mut(dragon).charging_player;
        let Some(target_location) = phase.target_location else {
            warn!("Aborting charge player as no target was set.");
            self.dragon_set_phase(dragon, EnderDragonPhase::HoldingPattern);
            return;
        };
        if phase.time_since_charge > 0 {
            let time_since_charge = phase.time_since_charge;
            phase.time_since_charge += 1;
            if time_since_charge >= CHARGE_RECOVERY_TIME {
                self.dragon_set_phase(dragon, EnderDragonPhase::HoldingPattern);
                return;
            }
        }
        let e = self.entity(dragon);
        let dist_to_target = target_location.distance_to_sqr(Vec3::new(e.x(), e.y(), e.z()));
        if dist_to_target < 100.0 || dist_to_target > 22500.0 || e.horizontal_collision || e.vertical_collision {
            self.dragon_phase_manager_mut(dragon).charging_player.time_since_charge += 1;
        }
    }
}
