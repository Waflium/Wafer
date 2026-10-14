use wafer_core::mth;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;

pub struct RangedAttackGoal {
    target: Option<EntityId>,
    attack_time: i32,
    speed_modifier: f64,
    see_time: i32,
    attack_interval_min: i32,
    attack_interval_max: i32,
    attack_radius: f32,
    attack_radius_sqr: f32,
}

impl RangedAttackGoal {
    pub fn new(speed_modifier: f64, attack_interval: i32, attack_radius: f32) -> RangedAttackGoal {
        RangedAttackGoal::with(speed_modifier, attack_interval, attack_interval, attack_radius)
    }

    pub fn with(speed_modifier: f64, attack_interval_min: i32, attack_interval_max: i32, attack_radius: f32) -> RangedAttackGoal {
        RangedAttackGoal {
            target: None,
            attack_time: -1,
            speed_modifier,
            see_time: 0,
            attack_interval_min,
            attack_interval_max,
            attack_radius,
            attack_radius_sqr: attack_radius * attack_radius,
        }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        match level.get_target(mob).filter(|&best_target| level.is_alive(best_target)) {
            Some(best_target) => {
                self.target = Some(best_target);
                true
            }
            None => false,
        }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        self.can_use(level, mob) || self.target.is_some_and(|target| level.is_alive(target)) && !level.mob(mob).expect("a mob").ai.navigation.is_done()
    }

    pub fn stop(&mut self) {
        self.target = None;
        self.see_time = 0;
        self.attack_time = -1;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = self.target else { return };
        let t = level.entity(target);
        let target_dist_sqr = level.entity(mob).distance_to_sqr_xyz(t.x(), t.y(), t.z());
        let has_line_of_sight = level.sensing_has_line_of_sight(mob, target);
        if has_line_of_sight {
            self.see_time += 1;
        } else {
            self.see_time = 0;
        }
        if !(target_dist_sqr > self.attack_radius_sqr as f64) && self.see_time >= 5 {
            level.mob_mut(mob).expect("a mob").ai.navigation.stop();
        } else {
            let speed_modifier = self.speed_modifier;
            level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, target, speed_modifier));
        }
        level.set_look_at_entity(mob, target, 30.0, 30.0);
        self.attack_time -= 1;
        if self.attack_time == 0 {
            if !has_line_of_sight {
                return;
            }
            let dist = target_dist_sqr.sqrt() as f32 / self.attack_radius;
            let power = mth::clamp_f(dist, 0.1, 1.0);
            level.perform_ranged_attack(mob, target, power);
            self.attack_time = mth::floor_f(dist * (self.attack_interval_max - self.attack_interval_min) as f32 + self.attack_interval_min as f32);
        } else if self.attack_time < 0 {
            self.attack_time = mth::floor(mth::lerp(target_dist_sqr.sqrt() / self.attack_radius as f64, self.attack_interval_min as f64, self.attack_interval_max as f64));
        }
    }
}
