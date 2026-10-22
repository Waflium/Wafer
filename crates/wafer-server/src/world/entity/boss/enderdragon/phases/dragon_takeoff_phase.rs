use wafer_core::Vec3;

use super::dragon_holding_pattern_phase::dragon_path_target;
use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::dimension::end::ender_dragon_fight;
use crate::world::level::pathfinder::path::Path;

#[derive(Default)]
pub struct DragonTakeoffPhase {
    pub first_tick: bool,
    pub current_path: Option<Path>,
    pub target_location: Option<Vec3>,
}

impl DragonTakeoffPhase {
    pub fn begin(&mut self) {
        self.first_tick = true;
        self.current_path = None;
        self.target_location = None;
    }
}

impl ServerLevel {
    fn takeoff(&mut self, dragon: EntityId) -> &mut DragonTakeoffPhase {
        &mut self.dragon_phase_manager_mut(dragon).takeoff
    }

    pub fn dragon_takeoff_do_server_tick(&mut self, dragon: EntityId) {
        let phase = self.takeoff(dragon);
        if !phase.first_tick && phase.current_path.is_some() {
            let fight_origin = self.ender_dragon(dragon).fight_origin;
            let egg = self.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, ender_dragon_fight::get_podium_location(fight_origin));
            if !egg.closer_to_center_than(self.entity(dragon).position(), 10.0) {
                self.dragon_set_phase(dragon, EnderDragonPhase::HoldingPattern);
            }
        } else {
            phase.first_tick = false;
            self.dragon_takeoff_find_new_target(dragon);
        }
    }

    fn dragon_takeoff_find_new_target(&mut self, dragon: EntityId) {
        let current_node_index = self.dragon_find_closest_node(dragon);
        let look_vector = self.ender_dragon_get_head_look_vector(dragon);
        let mut target_node_index = self.dragon_find_closest_node_to(dragon, -look_vector.x * 40.0, 105.0, -look_vector.z * 40.0);
        if self.dragon_alive_crystals(dragon).is_some_and(|crystals| crystals > 0) {
            target_node_index %= 12;
            if target_node_index < 0 {
                target_node_index += 12;
            }
        } else {
            target_node_index -= 12;
            target_node_index &= 7;
            target_node_index += 12;
        }
        let path = self.dragon_find_path(dragon, current_node_index, target_node_index, None);
        self.takeoff(dragon).current_path = path;
        self.dragon_takeoff_navigate_to_next_path_node(dragon);
    }

    fn dragon_takeoff_navigate_to_next_path_node(&mut self, dragon: EntityId) {
        let Some(path) = self.takeoff(dragon).current_path.as_mut() else { return };
        path.advance();
        if path.is_done() {
            return;
        }
        let current = path.get_next_node_pos();
        path.advance();
        let target_location = dragon_path_target(self, dragon, current);
        self.takeoff(dragon).target_location = Some(target_location);
    }
}
