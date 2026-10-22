use wafer_core::random::RandomSource;
use wafer_core::{Vec3, mth};

use super::dragon_holding_pattern_phase::dragon_path_target;
use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, LevelEntity};
use crate::world::entity::projectile::hurtingprojectile::dragon_fireball::DragonFireball;
use crate::world::level::pathfinder::node::Node;
use crate::world::level::pathfinder::path::Path;

const FIREBALL_CHARGE_AMOUNT: i32 = 5;

#[derive(Default)]
pub struct DragonStrafePlayerPhase {
    pub fireball_charge: i32,
    pub current_path: Option<Path>,
    pub target_location: Option<Vec3>,
    pub attack_target: Option<EntityId>,
    pub holding_pattern_clockwise: bool,
}

impl DragonStrafePlayerPhase {
    pub fn begin(&mut self) {
        self.fireball_charge = 0;
        self.target_location = None;
        self.current_path = None;
        self.attack_target = None;
    }
}

impl ServerLevel {
    fn strafe_player(&mut self, dragon: EntityId) -> &mut DragonStrafePlayerPhase {
        &mut self.dragon_phase_manager_mut(dragon).strafe_player
    }

    pub fn dragon_strafe_player_do_server_tick(&mut self, dragon: EntityId) {
        let Some(attack_target) = self.strafe_player(dragon).attack_target.filter(|&target| self.entity_ref(target).is_some()) else {
            warn!("Skipping player strafe phase because no player was found");
            self.dragon_set_phase(dragon, EnderDragonPhase::HoldingPattern);
            return;
        };
        if self.strafe_player(dragon).current_path.as_ref().is_some_and(Path::is_done) {
            let (t, d) = (self.entity(attack_target), self.entity(dragon));
            let (x_target, z_target) = (t.x(), t.z());
            let x_target_dist = x_target - d.x();
            let z_target_dist = z_target - d.z();
            let dist = (x_target_dist * x_target_dist + z_target_dist * z_target_dist).sqrt();
            let height_offset = (0.4f32 as f64 + dist / 80.0 - 1.0).min(10.0);
            let target_location = Vec3::new(x_target, t.y() + height_offset, z_target);
            self.strafe_player(dragon).target_location = Some(target_location);
        }
        let d = self.entity(dragon);
        let (x, y, z) = (d.x(), d.y(), d.z());
        let dist_to_target = self.strafe_player(dragon).target_location.map_or(0.0, |target| target.distance_to_sqr(Vec3::new(x, y, z)));
        if dist_to_target < 100.0 || dist_to_target > 22500.0 {
            self.dragon_strafe_player_find_new_target(dragon);
        }
        if self.entity(attack_target).distance_to_sqr(self.entity(dragon)) < 4096.0 {
            if self.has_line_of_sight(dragon, attack_target) {
                self.strafe_player(dragon).fireball_charge += 1;
                let (t, d) = (self.entity(attack_target), self.entity(dragon));
                let aim = Vec3::new(t.x() - d.x(), 0.0, t.z() - d.z()).normalize();
                let y_rot = d.y_rot * mth::DEG_TO_RAD;
                let dir = Vec3::new(mth::sin(y_rot as f64) as f64, 0.0, -mth::cos(y_rot as f64) as f64).normalize();
                let dot = dir.dot(aim) as f32;
                let mut angle_degs = ((dot as f64).acos() * 180.0f32 as f64 / std::f32::consts::PI as f64) as f32;
                angle_degs += 0.5;
                if self.strafe_player(dragon).fireball_charge >= FIREBALL_CHARGE_AMOUNT && (0.0..10.0).contains(&angle_degs) {
                    let view_vector = self.get_view_vector(dragon);
                    let head = self.entity(self.ender_dragon(dragon).head());
                    let starting_x = head.x() - view_vector.x;
                    let starting_y = head.y() + head.dimensions.height as f64 * 0.5 + 0.5;
                    let starting_z = head.z() - view_vector.z;
                    let t = self.entity(attack_target);
                    let xdd = t.x() - starting_x;
                    let ydd = t.y() + t.dimensions.height as f64 * 0.5 - starting_y;
                    let zdd = t.z() - starting_z;
                    let direction = Vec3::new(xdd, ydd, zdd);
                    if !self.entity(dragon).is_silent() {
                        let pos = self.entity(dragon).block_position();
                        self.level_event(None, crate::server::level::events::SOUND_DRAGON_FIREBALL, pos, 0);
                    }
                    let mut fireball = DragonFireball::from_mob(self, dragon, direction.normalize());
                    fireball.entity.snap_to(starting_x, starting_y, starting_z, 0.0, 0.0);
                    self.add_fresh_entity(LevelEntity::Projectile(Box::new(fireball)));
                    let phase = self.strafe_player(dragon);
                    phase.fireball_charge = 0;
                    if let Some(path) = &mut phase.current_path {
                        while !path.is_done() {
                            path.advance();
                        }
                    }
                    self.dragon_set_phase(dragon, EnderDragonPhase::HoldingPattern);
                }
            } else if self.strafe_player(dragon).fireball_charge > 0 {
                self.strafe_player(dragon).fireball_charge -= 1;
            }
        } else if self.strafe_player(dragon).fireball_charge > 0 {
            self.strafe_player(dragon).fireball_charge -= 1;
        }
    }

    fn dragon_strafe_player_find_new_target(&mut self, dragon: EntityId) {
        if self.strafe_player(dragon).current_path.as_ref().is_none_or(Path::is_done) {
            let current_node_index = self.dragon_find_closest_node(dragon);
            let mut target_node_index = current_node_index;
            if self.entity_mut(dragon).random.next_int_bounded(8) == 0 {
                let phase = self.strafe_player(dragon);
                phase.holding_pattern_clockwise = !phase.holding_pattern_clockwise;
                target_node_index += 6;
            }
            if self.strafe_player(dragon).holding_pattern_clockwise {
                target_node_index += 1;
            } else {
                target_node_index -= 1;
            }
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
            let mut path = self.dragon_find_path(dragon, current_node_index, target_node_index, None);
            if let Some(path) = &mut path {
                path.advance();
            }
            self.strafe_player(dragon).current_path = path;
        }
        self.dragon_strafe_player_navigate_to_next_path_node(dragon);
    }

    fn dragon_strafe_player_navigate_to_next_path_node(&mut self, dragon: EntityId) {
        let Some(path) = self.strafe_player(dragon).current_path.as_mut().filter(|path| !path.is_done()) else { return };
        let current = path.get_next_node_pos();
        path.advance();
        let target_location = dragon_path_target(self, dragon, current);
        self.strafe_player(dragon).target_location = Some(target_location);
    }

    pub fn dragon_strafe_player_set_target(&mut self, dragon: EntityId, target: EntityId) {
        self.strafe_player(dragon).attack_target = Some(target);
        let current_node_index = self.dragon_find_closest_node(dragon);
        let t = self.entity(target);
        let (tx, ty, tz) = (t.x(), t.y(), t.z());
        let target_node_index = self.dragon_find_closest_node_to(dragon, tx, ty, tz);
        let block = self.entity(target).block_position();
        let (final_x_target, final_z_target) = (block.x, block.z);
        let d = self.entity(dragon);
        let xd = final_x_target as f64 - d.x();
        let zd = final_z_target as f64 - d.z();
        let sd = (xd * xd + zd * zd).sqrt();
        let ho = (0.4f32 as f64 + sd / 80.0 - 1.0).min(10.0);
        let final_y_target = mth::floor(ty + ho);
        let final_node = Node::new(final_x_target, final_y_target, final_z_target);
        let path = self.dragon_find_path(dragon, current_node_index, target_node_index, Some(final_node));
        self.strafe_player(dragon).current_path = path;
        if let Some(path) = &mut self.strafe_player(dragon).current_path {
            path.advance();
        } else {
            return;
        }
        self.dragon_strafe_player_navigate_to_next_path_node(dragon);
    }
}
