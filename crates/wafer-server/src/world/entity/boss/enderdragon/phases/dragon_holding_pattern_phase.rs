use wafer_core::random::RandomSource;
use wafer_core::{BlockPos, Vec3};

use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;
use crate::world::level::chunk::HeightmapType;
use crate::world::level::server_level_accessor::ServerLevelAccessor;
use crate::world::level::dimension::end::ender_dragon_fight;
use crate::world::level::pathfinder::path::Path;

const NEW_TARGET_TARGETING: TargetingConditions<'static> = TargetingConditions::for_combat().ignore_line_of_sight();

#[derive(Default)]
pub struct DragonHoldingPatternPhase {
    pub current_path: Option<Path>,
    pub target_location: Option<Vec3>,
    pub clockwise: bool,
}

impl DragonHoldingPatternPhase {
    pub fn begin(&mut self) {
        self.current_path = None;
        self.target_location = None;
    }
}

impl ServerLevel {
    fn holding_pattern(&mut self, dragon: EntityId) -> &mut DragonHoldingPatternPhase {
        &mut self.dragon_phase_manager_mut(dragon).holding_pattern
    }

    pub fn dragon_holding_pattern_do_server_tick(&mut self, dragon: EntityId) {
        let e = self.entity(dragon);
        let (x, y, z, horizontal_collision, vertical_collision) = (e.x(), e.y(), e.z(), e.horizontal_collision, e.vertical_collision);
        let dist_to_target = self.holding_pattern(dragon).target_location.map_or(0.0, |target| target.distance_to_sqr(Vec3::new(x, y, z)));
        if dist_to_target < 100.0 || dist_to_target > 22500.0 || horizontal_collision || vertical_collision {
            self.dragon_holding_pattern_find_new_target(dragon);
        }
    }

    fn dragon_holding_pattern_find_new_target(&mut self, dragon: EntityId) {
        if self.holding_pattern(dragon).current_path.as_ref().is_some_and(Path::is_done) {
            let fight_origin = self.ender_dragon(dragon).fight_origin;
            let egg = self.get_heightmap_pos(HeightmapType::MotionBlockingNoLeaves, ender_dragon_fight::get_podium_location(fight_origin));
            let crystals = self.dragon_alive_crystals(dragon).unwrap_or(0);
            if self.entity_mut(dragon).random.next_int_bounded(crystals + 3) == 0 {
                self.dragon_set_phase(dragon, EnderDragonPhase::LandingApproach);
                return;
            }
            let player_nearest_to_egg = self.get_nearest_player(&NEW_TARGET_TARGETING, Some(dragon), egg.x as f64, egg.y as f64, egg.z as f64);
            let dist_sqr = match player_nearest_to_egg {
                Some(player) => {
                    let p = self.entity(player).position();
                    egg.dist_to_center_sqr(p.x, p.y, p.z) / 512.0
                }
                None => 64.0,
            };
            if let Some(player) = player_nearest_to_egg
                && (self.entity_mut(dragon).random.next_int_bounded((dist_sqr + 2.0) as i32) == 0 || self.entity_mut(dragon).random.next_int_bounded(crystals + 2) == 0)
            {
                self.dragon_holding_pattern_strafe_player(dragon, player);
                return;
            }
        }
        if self.holding_pattern(dragon).current_path.as_ref().is_none_or(Path::is_done) {
            let current_node_index = self.dragon_find_closest_node(dragon);
            let mut target_node_index = current_node_index;
            if self.entity_mut(dragon).random.next_int_bounded(8) == 0 {
                let phase = self.holding_pattern(dragon);
                phase.clockwise = !phase.clockwise;
                target_node_index += 6;
            }
            if self.holding_pattern(dragon).clockwise {
                target_node_index += 1;
            } else {
                target_node_index -= 1;
            }
            if self.dragon_alive_crystals(dragon).is_some_and(|crystals| crystals >= 0) {
                target_node_index %= 12;
                if target_node_index < 0 {
                    target_node_index += 12;
                }
            } else {
                target_node_index -= 12;
                target_node_index &= 7;
                target_node_index += 12;
            }
            let mut path = self.dragon_find_path(dragon, current_node_index, target_node_index, None);
            if let Some(path) = &mut path {
                path.advance();
            }
            self.holding_pattern(dragon).current_path = path;
        }
        self.dragon_holding_pattern_navigate_to_next_path_node(dragon);
    }

    fn dragon_holding_pattern_strafe_player(&mut self, dragon: EntityId, player_nearest_to_egg: EntityId) {
        self.dragon_set_phase(dragon, EnderDragonPhase::StrafePlayer);
        self.dragon_strafe_player_set_target(dragon, player_nearest_to_egg);
    }

    fn dragon_holding_pattern_navigate_to_next_path_node(&mut self, dragon: EntityId) {
        let Some(path) = self.holding_pattern(dragon).current_path.as_mut().filter(|path| !path.is_done()) else { return };
        let current = path.get_next_node_pos();
        path.advance();
        let target_location = dragon_path_target(self, dragon, current);
        self.holding_pattern(dragon).target_location = Some(target_location);
    }

    pub fn dragon_holding_pattern_on_crystal_destroyed(&mut self, dragon: EntityId, player: Option<EntityId>) {
        if let Some(player) = player
            && self.ender_dragon_can_attack(player)
        {
            self.dragon_holding_pattern_strafe_player(dragon, player);
        }
    }
}

pub fn dragon_path_target(level: &mut ServerLevel, dragon: EntityId, current: BlockPos) -> Vec3 {
    let (x_target, z_target) = (current.x as f64, current.z as f64);
    let mut y_target;
    loop {
        y_target = current.y as f64 + (level.entity_mut(dragon).random.next_float() * 20.0) as f64;
        if y_target >= current.y as f64 {
            break;
        }
    }
    Vec3::new(x_target, y_target, z_target)
}
