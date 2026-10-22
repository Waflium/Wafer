use wafer_core::mth;

use crate::server::level::ServerLevel;
use crate::world::entity::{Entity, EntityId};

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum LookControlKind {
    #[default]
    Default,
    SmoothSwimming(i32),
    Axolotl(i32),
    Frog,
    Fox,
    Phantom,
    Bee,
    Shulker,
    Camel,
    HappyGhast,
}

#[derive(Default)]
pub struct LookControl {
    pub kind: LookControlKind,
    pub y_max_rot_speed: f32,
    pub x_max_rot_angle: f32,
    pub look_at_cooldown: i32,
    pub wanted_x: f64,
    pub wanted_y: f64,
    pub wanted_z: f64,
}

impl LookControl {
    pub fn set_look_at(&mut self, x: f64, y: f64, z: f64, y_max_rot_speed: f32, x_max_rot_angle: f32) {
        self.wanted_x = x;
        self.wanted_y = y;
        self.wanted_z = z;
        self.y_max_rot_speed = y_max_rot_speed;
        self.x_max_rot_angle = x_max_rot_angle;
        self.look_at_cooldown = 2;
    }

    pub fn is_looking_at_target(&self) -> bool {
        self.look_at_cooldown > 0
    }

    pub fn get_x_rot_d(&self, e: &Entity) -> Option<f32> {
        let xd = self.wanted_x - e.x();
        let yd = self.wanted_y - e.eye_y();
        let zd = self.wanted_z - e.z();
        let sd = (xd * xd + zd * zd).sqrt();
        if !(yd.abs() > 1.0e-5f32 as f64) && !(sd.abs() > 1.0e-5f32 as f64) { None } else { Some((-(mth::atan2(yd, sd) * mth::RAD_TO_DEG as f64)) as f32) }
    }

    pub fn get_y_rot_d(&self, e: &Entity) -> Option<f32> {
        let xd = self.wanted_x - e.x();
        let zd = self.wanted_z - e.z();
        if !(zd.abs() > 1.0e-5f32 as f64) && !(xd.abs() > 1.0e-5f32 as f64) { None } else { Some((mth::atan2(zd, xd) * mth::RAD_TO_DEG as f64) as f32 - 90.0) }
    }
}

pub fn rotate_towards(from_angle: f32, to_angle: f32, max_rot: f32) -> f32 {
    let diff = mth::degrees_difference(from_angle, to_angle);
    from_angle + mth::clamp_f(diff, -max_rot, max_rot)
}

impl ServerLevel {
    pub fn set_look_at(&mut self, id: EntityId, x: f64, y: f64, z: f64) {
        let mob = self.mob_mut(id).expect("a mob");
        let (head_rot_speed, max_head_x_rot) = (mob.get_head_rot_speed() as f32, mob.get_max_head_x_rot() as f32);
        mob.ai.look_control.set_look_at(x, y, z, head_rot_speed, max_head_x_rot);
    }

    pub fn set_look_at_entity(&mut self, id: EntityId, target: EntityId, y_max_rot_speed: f32, x_max_rot_angle: f32) {
        let t = self.entity(target);
        let (x, y, z) = (t.x(), t.eye_y(), t.z());
        self.mob_mut(id).expect("a mob").ai.look_control.set_look_at(x, y, z, y_max_rot_speed, x_max_rot_angle);
    }

    pub fn look_control_tick(&mut self, id: EntityId) {
        match self.mob(id).expect("a mob").ai.look_control.kind {
            LookControlKind::Default | LookControlKind::Frog => self.look_control_look_control_tick(id),
            LookControlKind::SmoothSwimming(max_y_rot_from_center) => self.smooth_swimming_look_control_tick(id, max_y_rot_from_center),
            LookControlKind::Axolotl(max_y_rot_from_center) => self.axolotl_look_control_tick(id, max_y_rot_from_center),
            LookControlKind::Fox => self.fox_look_control_tick(id),
            LookControlKind::Phantom => {}
            LookControlKind::Shulker => self.shulker_look_control_tick(id),
            LookControlKind::Camel => self.camel_look_control_tick(id),
            LookControlKind::HappyGhast => self.happy_ghast_look_control_tick(id),
            LookControlKind::Bee => {
                if !self.is_angry(id) {
                    self.look_control_look_control_tick(id);
                }
            }
        }
    }

    fn reset_x_rot_on_tick(&self, id: EntityId) -> bool {
        match self.mob(id).expect("a mob").ai.look_control.kind {
            LookControlKind::Frog => self.frog_look_control_reset_x_rot_on_tick(id),
            LookControlKind::Fox => self.fox_look_control_reset_x_rot_on_tick(id),
            LookControlKind::Bee => !crate::world::entity::animal::bee::bee::is_pollinating(self.mob(id).expect("a bee")),
            _ => true,
        }
    }

    pub fn look_control_look_control_tick(&mut self, id: EntityId) {
        if self.reset_x_rot_on_tick(id) {
            self.entity_mut(id).set_x_rot(0.0);
        }
        let mob = self.mob_mut(id).expect("a mob");
        let navigation_done = mob.ai.navigation.is_done();
        let max_head_y_rot = mob.max_head_y_rot() as f32;
        let look = &mut mob.ai.look_control;
        let e = &mut mob.entity;
        if look.look_at_cooldown > 0 {
            look.look_at_cooldown -= 1;
            if let Some(y_rot_d) = look.get_y_rot_d(e) {
                e.y_head_rot = rotate_towards(e.y_head_rot, y_rot_d, look.y_max_rot_speed);
            }
            if let Some(x_rot_d) = look.get_x_rot_d(e) {
                let x_rot = rotate_towards(e.x_rot, x_rot_d, look.x_max_rot_angle);
                e.set_x_rot(x_rot);
            }
        } else {
            e.y_head_rot = rotate_towards(e.y_head_rot, mob.living.y_body_rot, 10.0);
        }
        if !navigation_done {
            e.y_head_rot = mth::rotate_if_necessary(e.y_head_rot, mob.living.y_body_rot, max_head_y_rot);
        }
    }
}
