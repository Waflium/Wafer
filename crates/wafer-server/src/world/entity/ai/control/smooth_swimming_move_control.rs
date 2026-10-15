use wafer_core::mth;

use super::look_control::rotate_towards;
use super::move_control::{Operation, rotlerp};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;

const FULL_SPEED_TURN_THRESHOLD: f32 = 10.0;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct SmoothSwimmingMoveControl {
    max_turn_x: i32,
    max_turn_y: i32,
    in_water_speed_modifier: f32,
    outside_water_speed_modifier: f32,
    apply_gravity: bool,
}

impl SmoothSwimmingMoveControl {
    pub const fn new(max_turn_x: i32, max_turn_y: i32, in_water_speed_modifier: f32, outside_water_speed_modifier: f32, apply_gravity: bool) -> SmoothSwimmingMoveControl {
        SmoothSwimmingMoveControl { max_turn_x, max_turn_y, in_water_speed_modifier, outside_water_speed_modifier, apply_gravity }
    }
}

fn get_turning_speed_factor(left_to_turn: f32) -> f32 {
    1.0 - mth::clamp_f((left_to_turn - FULL_SPEED_TURN_THRESHOLD) / 50.0, 0.0, 1.0)
}

impl ServerLevel {
    pub fn smooth_swimming_move_control_tick(&mut self, id: EntityId, control: SmoothSwimmingMoveControl) {
        if control.apply_gravity && self.entity(id).is_in_water() {
            let e = self.entity_mut(id);
            e.set_delta_movement(e.delta_movement.add(0.0, 0.005, 0.0));
        }
        let movement_speed = self.attribute_value(id, Attribute::MovementSpeed);
        let mob = self.mob_mut(id).expect("a mob");
        if mob.ai.move_control.operation == Operation::MoveTo && !mob.ai.navigation.is_done() {
            let move_control = &mob.ai.move_control;
            let e = &mob.entity;
            let xd = move_control.wanted_x - e.x();
            let yd = move_control.wanted_y - e.y();
            let zd = move_control.wanted_z - e.z();
            let dd = xd * xd + yd * yd + zd * zd;
            if dd < 2.5000003e-7f32 as f64 {
                mob.living.zza = 0.0;
            } else {
                let y_rot_d = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
                let y_rot = rotlerp(mob.entity.y_rot, y_rot_d, control.max_turn_y as f32);
                mob.entity.set_y_rot(y_rot);
                mob.living.y_body_rot = mob.entity.y_rot;
                mob.entity.y_head_rot = mob.entity.y_rot;
                let speed = (mob.ai.move_control.speed_modifier * movement_speed) as f32;
                if mob.entity.is_in_water() {
                    mob.set_speed(speed * control.in_water_speed_modifier);
                    let sqrt = (xd * xd + zd * zd).sqrt();
                    if yd.abs() > 1.0e-5f32 as f64 || sqrt.abs() > 1.0e-5f32 as f64 {
                        let x_rot_d = -((mth::atan2(yd, sqrt) * mth::RAD_TO_DEG as f64) as f32);
                        let x_rot_d = mth::clamp_f(mth::wrap_degrees(x_rot_d), -control.max_turn_x as f32, control.max_turn_x as f32);
                        let x_rot = rotate_towards(mob.entity.x_rot, x_rot_d, 5.0);
                        mob.entity.set_x_rot(x_rot);
                    }
                    let radians = mob.entity.x_rot * (std::f64::consts::PI / 180.0) as f32;
                    let cos = mth::cos(radians as f64);
                    let sin = mth::sin(radians as f64);
                    mob.living.zza = cos * speed;
                    mob.living.yya = -sin * speed;
                } else {
                    let left_to_turn = mth::wrap_degrees(mob.entity.y_rot - y_rot_d).abs();
                    let turning_speed_factor = get_turning_speed_factor(left_to_turn);
                    mob.set_speed(speed * control.outside_water_speed_modifier * turning_speed_factor);
                }
            }
        } else {
            mob.set_speed(0.0);
            mob.living.xxa = 0.0;
            mob.living.yya = 0.0;
            mob.living.zza = 0.0;
        }
    }
}
