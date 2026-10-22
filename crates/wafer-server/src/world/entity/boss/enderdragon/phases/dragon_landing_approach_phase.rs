use wafer_core::Vec3;

use super::dragon_holding_pattern_phase::dragon_path_target;
use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::dimension::end::ender_dragon_fight;
use crate::world::level::pathfinder::node::Node;
use crate::world::level::pathfinder::path::Path;

const NEAR_EGG_TARGETING: TargetingConditions<'static> = TargetingConditions::for_combat().ignore_line_of_sight();

#[derive(Default)]
pub struct DragonLandingApproachPhase {
    pub current_path: Option<Path>,
    pub target_location: Option<Vec3>,
}

impl DragonLandingApproachPhase {
    pub fn begin(&mut self) {
        self.current_path = None;
        self.target_location = None;
    }
}

impl ServerLevel {
    fn landing_approach(&mut self, dragon: EntityId) -> &mut DragonLandingApproachPhase {
        &mut self.dragon_phase_manager_mut(dragon).landing_approach
    }

    pub fn dragon_landing_approach_do_server_tick(&mut self, dragon: EntityId) {
        let e = self.entity(dragon);
        let (x, y, z, horizontal_collision, vertical_collision) = (e.x(), e.y(), e.z(), e.horizontal_collision, e.vertical_collision);
        let dist_to_target = self.landing_approach(dragon).target_location.map_or(0.0, |target| target.distance_to_sqr(Vec3::new(x, y, z)));
        if dist_to_target < 100.0 || dist_to_target > 22500.0 || horizontal_collision || vertical_collision {
            self.dragon_landing_approach_find_new_target(dragon);
        }
    }

    fn dragon_landing_approach_find_new_target(&mut self, dragon: EntityId) {
        if self.landing_approach(dragon).current_path.as_ref().is_none_or(Path::is_done) {
            let current_node_index = self.dragon_find_closest_node(dragon);
            let fight_origin = self.ender_dragon(dragon).fight_origin;
            let egg = self.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, ender_dragon_fight::get_podium_location(fight_origin));
            let player_nearest_to_egg = self.get_nearest_player(&NEAR_EGG_TARGETING, Some(dragon), egg.x as f64, egg.y as f64, egg.z as f64);
            let target_node_index = match player_nearest_to_egg {
                Some(player) => {
                    let p = self.entity(player);
                    let aim = Vec3::new(p.x(), 0.0, p.z()).normalize();
                    self.dragon_find_closest_node_to(dragon, -aim.x * 40.0, 105.0, -aim.z * 40.0)
                }
                None => self.dragon_find_closest_node_to(dragon, 40.0, egg.y as f64, 0.0),
            };
            let final_node = Node::new(egg.x, egg.y, egg.z);
            let mut path = self.dragon_find_path(dragon, current_node_index, target_node_index, Some(final_node));
            if let Some(path) = &mut path {
                path.advance();
            }
            self.landing_approach(dragon).current_path = path;
        }
        self.dragon_landing_approach_navigate_to_next_path_node(dragon);
        if self.landing_approach(dragon).current_path.as_ref().is_some_and(Path::is_done) {
            self.dragon_set_phase(dragon, EnderDragonPhase::Landing);
        }
    }

    fn dragon_landing_approach_navigate_to_next_path_node(&mut self, dragon: EntityId) {
        let Some(path) = self.landing_approach(dragon).current_path.as_mut().filter(|path| !path.is_done()) else { return };
        let current = path.get_next_node_pos();
        path.advance();
        let target_location = dragon_path_target(self, dragon, current);
        self.landing_approach(dragon).target_location = Some(target_location);
    }
}
