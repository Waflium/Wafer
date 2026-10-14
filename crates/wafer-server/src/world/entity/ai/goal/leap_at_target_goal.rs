use wafer_core::Vec3;
use wafer_core::random::RandomSource;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct LeapAtTargetGoal {
    target: Option<EntityId>,
    yd: f32,
}

impl LeapAtTargetGoal {
    pub fn new(yd: f32) -> LeapAtTargetGoal {
        LeapAtTargetGoal { target: None, yd }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        if level.has_controlling_passenger(mob) {
            return false;
        }
        self.target = level.get_target(mob);
        let Some(target) = self.target else { return false };
        let d = level.entity(mob).distance_to_sqr(level.entity(target));
        if !(4.0..=16.0).contains(&d) {
            return false;
        }
        let e = level.entity_mut(mob);
        e.on_ground && e.random.next_int_bounded(reduced_tick_delay(5)) == 0
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        !level.entity(mob).on_ground
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let target = self.target.expect("a leap target");
        let (target_x, target_z) = (level.entity(target).x(), level.entity(target).z());
        let e = level.entity_mut(mob);
        let movement = e.delta_movement;
        let mut delta = Vec3::new(target_x - e.x(), 0.0, target_z - e.z());
        if delta.length_sqr() > 1.0e-7 {
            delta = delta.normalize().scale(0.4).add_vec(movement.scale(0.2));
        }
        e.set_delta_movement(Vec3::new(delta.x, self.yd as f64, delta.z));
    }
}
