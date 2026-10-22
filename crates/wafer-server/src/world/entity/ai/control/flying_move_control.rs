use wafer_core::mth;

use super::move_control::{Operation, rotlerp};
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::attributes::Attribute;

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct FlyingMoveControl {
    max_turn: i32,
    hovers_in_place: bool,
}

impl FlyingMoveControl {
    pub const fn new(max_turn: i32, hovers_in_place: bool) -> FlyingMoveControl {
        FlyingMoveControl { max_turn, hovers_in_place }
    }
}

impl ServerLevel {
    pub fn flying_move_control_tick(&mut self, id: EntityId, control: FlyingMoveControl) {
        let on_ground = self.entity(id).on_ground;
        let speed_attribute = if on_ground { self.attribute_value(id, Attribute::MovementSpeed) } else { self.attribute_value(id, Attribute::FlyingSpeed) };
        let mob = self.mob_mut(id).expect("a mob");
        if mob.ai.move_control.operation == Operation::MoveTo {
            mob.ai.move_control.operation = Operation::Wait;
            mob.entity.set_no_gravity(true);
            let move_control = &mob.ai.move_control;
            let xd = move_control.wanted_x - mob.entity.x();
            let yd = move_control.wanted_y - mob.entity.y();
            let zd = move_control.wanted_z - mob.entity.z();
            let dd = xd * xd + yd * yd + zd * zd;
            if dd < 2.5000003e-7f32 as f64 {
                mob.living.yya = 0.0;
                mob.living.zza = 0.0;
                return;
            }
            let y_rot_d = (mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0;
            let y_rot = rotlerp(mob.entity.y_rot, y_rot_d, 90.0);
            mob.entity.set_y_rot(y_rot);
            let speed = (move_control.speed_modifier * speed_attribute) as f32;
            mob.set_speed(speed);
            let sd = (xd * xd + zd * zd).sqrt();
            if yd.abs() > 1.0e-5f32 as f64 || sd.abs() > 1.0e-5f32 as f64 {
                let x_rot_d = (-(mth::atan2(yd, sd) * mth::RAD_TO_DEG as f64)) as f32;
                let x_rot = rotlerp(mob.entity.x_rot, x_rot_d, control.max_turn as f32);
                mob.entity.set_x_rot(x_rot);
                mob.living.yya = if yd > 0.0 { speed } else { -speed };
            }
        } else {
            if !control.hovers_in_place {
                mob.entity.set_no_gravity(false);
            }
            mob.living.yya = 0.0;
            mob.living.zza = 0.0;
        }
    }
}
