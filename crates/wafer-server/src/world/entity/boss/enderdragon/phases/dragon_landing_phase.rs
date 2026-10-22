use wafer_core::Vec3;

use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::dimension::end::ender_dragon_fight;

pub const FLY_SPEED: f32 = 1.5;

#[derive(Default)]
pub struct DragonLandingPhase {
    pub target_location: Option<Vec3>,
}

impl DragonLandingPhase {
    pub fn begin(&mut self) {
        self.target_location = None;
    }
}

impl ServerLevel {
    pub fn dragon_landing_do_server_tick(&mut self, dragon: EntityId) {
        if self.dragon_phase_manager(dragon).landing.target_location.is_none() {
            let fight_origin = self.ender_dragon(dragon).fight_origin;
            let pos = self.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, ender_dragon_fight::get_podium_location(fight_origin));
            self.dragon_phase_manager_mut(dragon).landing.target_location = Some(Vec3::at_bottom_center_of(pos));
        }
        let target_location = self.dragon_phase_manager(dragon).landing.target_location.expect("a landing target");
        let e = self.entity(dragon);
        if target_location.distance_to_sqr(Vec3::new(e.x(), e.y(), e.z())) < 1.0 {
            self.dragon_phase_manager_mut(dragon).sitting_flaming.reset_flame_count();
            self.dragon_set_phase(dragon, EnderDragonPhase::SittingScanning);
        }
    }

    pub fn dragon_landing_get_turn_speed(&self, dragon: EntityId) -> f32 {
        let rot_speed = self.entity(dragon).delta_movement.horizontal_distance() as f32 + 1.0;
        let dist = rot_speed.min(40.0);
        dist / rot_speed
    }
}
