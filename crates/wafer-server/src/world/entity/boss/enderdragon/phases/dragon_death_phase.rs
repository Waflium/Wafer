use wafer_core::Vec3;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::dimension::end::ender_dragon_fight;

pub const FLY_SPEED: f32 = 3.0;

#[derive(Default)]
pub struct DragonDeathPhase {
    pub target_location: Option<Vec3>,
    pub time: i32,
}

impl DragonDeathPhase {
    pub fn begin(&mut self) {
        self.target_location = None;
        self.time = 0;
    }
}

impl ServerLevel {
    pub fn dragon_death_do_server_tick(&mut self, dragon: EntityId) {
        self.dragon_phase_manager_mut(dragon).dying.time += 1;
        if self.dragon_phase_manager(dragon).dying.target_location.is_none() {
            let fight_origin = self.ender_dragon(dragon).fight_origin;
            let egg = self.get_heightmap_pos(HeightmapType::MotionBlocking, ender_dragon_fight::get_podium_location(fight_origin));
            self.dragon_phase_manager_mut(dragon).dying.target_location = Some(Vec3::at_bottom_center_of(egg));
        }
        let target_location = self.dragon_phase_manager(dragon).dying.target_location.expect("a death target");
        let e = self.entity(dragon);
        let dist_to_target = target_location.distance_to_sqr(Vec3::new(e.x(), e.y(), e.z()));
        if !(dist_to_target < 100.0) && !(dist_to_target > 22500.0) && !e.horizontal_collision && !e.vertical_collision {
            self.set_health(dragon, 1.0);
        } else {
            self.set_health(dragon, 0.0);
        }
    }
}
