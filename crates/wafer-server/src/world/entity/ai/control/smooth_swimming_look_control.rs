use wafer_core::mth;

use super::look_control::rotate_towards;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

const HEAD_TILT_X: f32 = 10.0;
const HEAD_TILT_Y: f32 = 20.0;

impl ServerLevel {
    pub fn smooth_swimming_look_control_tick(&mut self, id: EntityId, max_y_rot_from_center: i32) {
        let mob = self.mob_mut(id).expect("a mob");
        let navigation_done = mob.ai.navigation.is_done();
        let look = &mut mob.ai.look_control;
        let e = &mut mob.entity;
        if look.look_at_cooldown > 0 {
            look.look_at_cooldown -= 1;
            if let Some(y_rot_d) = look.get_y_rot_d(e) {
                e.y_head_rot = rotate_towards(e.y_head_rot, y_rot_d + HEAD_TILT_Y, look.y_max_rot_speed);
            }
            if let Some(x_rot_d) = look.get_x_rot_d(e) {
                let x_rot = rotate_towards(e.x_rot, x_rot_d + HEAD_TILT_X, look.x_max_rot_angle);
                e.set_x_rot(x_rot);
            }
        } else {
            if navigation_done {
                let x_rot = rotate_towards(e.x_rot, 0.0, 5.0);
                e.set_x_rot(x_rot);
            }
            e.y_head_rot = rotate_towards(e.y_head_rot, mob.living.y_body_rot, look.y_max_rot_speed);
        }
        let head_diff_body = mth::wrap_degrees(e.y_head_rot - mob.living.y_body_rot);
        if head_diff_body < -max_y_rot_from_center as f32 {
            mob.living.y_body_rot -= 4.0;
        } else if head_diff_body > max_y_rot_from_center as f32 {
            mob.living.y_body_rot += 4.0;
        }
    }
}
