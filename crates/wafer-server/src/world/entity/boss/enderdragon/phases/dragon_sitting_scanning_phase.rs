use wafer_core::{Vec3, mth};

use super::ender_dragon_phase::EnderDragonPhase;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::ai::targeting::targeting_conditions::TargetingConditions;

const SITTING_SCANNING_IDLE_TICKS: i32 = 100;
const SITTING_ATTACK_Y_VIEW_RANGE: f64 = 10.0;
const SITTING_ATTACK_VIEW_RANGE: f64 = 20.0;
const SITTING_CHARGE_VIEW_RANGE: f64 = 150.0;
const CHARGE_TARGETING: TargetingConditions<'static> = TargetingConditions::for_combat().range(SITTING_CHARGE_VIEW_RANGE);

#[derive(Default)]
pub struct DragonSittingScanningPhase {
    pub scanning_time: i32,
}

impl DragonSittingScanningPhase {
    pub fn begin(&mut self) {
        self.scanning_time = 0;
    }
}

impl ServerLevel {
    pub fn dragon_sitting_scanning_do_server_tick(&mut self, dragon: EntityId) {
        self.dragon_phase_manager_mut(dragon).sitting_scanning.scanning_time += 1;
        let d = self.entity(dragon);
        let (x, y, z) = (d.x(), d.y(), d.z());
        let selector = move |target: EntityId, level: &ServerLevel| (level.entity(target).y() - level.entity(dragon).y()).abs() <= SITTING_ATTACK_Y_VIEW_RANGE;
        let scan_targeting = TargetingConditions::for_combat().range(SITTING_ATTACK_VIEW_RANGE).selector(&selector);
        let attack_target = self.get_nearest_player(&scan_targeting, Some(dragon), x, y, z);
        let scanning_time = self.dragon_phase_manager(dragon).sitting_scanning.scanning_time;
        if let Some(attack_target) = attack_target {
            if scanning_time > 25 {
                self.dragon_set_phase(dragon, EnderDragonPhase::SittingAttacking);
            } else {
                let (t, d) = (self.entity(attack_target), self.entity(dragon));
                let aim = Vec3::new(t.x() - d.x(), 0.0, t.z() - d.z()).normalize();
                let y_rot = d.y_rot * mth::DEG_TO_RAD;
                let dir = Vec3::new(mth::sin(y_rot as f64) as f64, 0.0, -mth::cos(y_rot as f64) as f64).normalize();
                let dot = dir.dot(aim) as f32;
                let angle = ((dot as f64).acos() * 180.0f32 as f64 / std::f32::consts::PI as f64) as f32 + 0.5;
                if !(0.0..=10.0).contains(&angle) {
                    let head = self.entity(self.ender_dragon(dragon).head());
                    let x_attack_dist = t.x() - head.x();
                    let z_attack_dist = t.z() - head.z();
                    let y_rot_delta = mth::clamp(
                        mth::wrap_degrees_d(180.0 - mth::atan2(x_attack_dist, z_attack_dist) * 180.0f32 as f64 / std::f32::consts::PI as f64 - d.y_rot as f64),
                        -100.0,
                        100.0,
                    );
                    let mut dist = (x_attack_dist * x_attack_dist + z_attack_dist * z_attack_dist).sqrt() as f32 + 1.0;
                    let rot_speed = dist;
                    if dist > 40.0 {
                        dist = 40.0;
                    }
                    let dragon_state = self.ender_dragon_mut(dragon);
                    dragon_state.y_rot_a *= 0.8;
                    dragon_state.y_rot_a += y_rot_delta as f32 * (0.7 / dist / rot_speed);
                    let y_rot_a = dragon_state.y_rot_a;
                    let e = self.entity_mut(dragon);
                    e.set_y_rot(e.y_rot + y_rot_a);
                }
            }
        } else if scanning_time >= SITTING_SCANNING_IDLE_TICKS {
            let attack_target = self.get_nearest_player(&CHARGE_TARGETING, Some(dragon), x, y, z);
            self.dragon_set_phase(dragon, EnderDragonPhase::Takeoff);
            if let Some(attack_target) = attack_target {
                self.dragon_set_phase(dragon, EnderDragonPhase::ChargingPlayer);
                let t = self.entity(attack_target);
                let target = Vec3::new(t.x(), t.y(), t.z());
                self.dragon_phase_manager_mut(dragon).charging_player.target_location = Some(target);
            }
        }
    }
}
