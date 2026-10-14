use wafer_core::random::RandomSource;
use wafer_data::items;

use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::projectile_util;
use crate::world::item::bow_item;

pub struct RangedBowAttackGoal {
    speed_modifier: f64,
    attack_interval_min: i32,
    attack_radius_sqr: f32,
    attack_time: i32,
    see_time: i32,
    strafing_clockwise: bool,
    strafing_backwards: bool,
    strafing_time: i32,
}

impl RangedBowAttackGoal {
    pub fn new(speed_modifier: f64, attack_interval_min: i32, attack_radius: f32) -> RangedBowAttackGoal {
        RangedBowAttackGoal {
            speed_modifier,
            attack_interval_min,
            attack_radius_sqr: attack_radius * attack_radius,
            attack_time: -1,
            see_time: 0,
            strafing_clockwise: false,
            strafing_backwards: false,
            strafing_time: -1,
        }
    }

    pub fn set_min_attack_interval(&mut self, ticks: i32) {
        self.attack_interval_min = ticks;
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        if level.get_target(mob).is_none() { false } else { self.is_holding_bow(level, mob) }
    }

    fn is_holding_bow(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.is_holding(mob, items::BOW)
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        (self.can_use(level, mob) || !level.mob(mob).expect("a mob").ai.navigation.is_done()) && self.is_holding_bow(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").set_aggressive(true);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").set_aggressive(false);
        self.see_time = 0;
        self.attack_time = -1;
        level.stop_using_item(mob);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
        let t = level.entity(target);
        let target_dist_sqr = level.entity(mob).distance_to_sqr_xyz(t.x(), t.y(), t.z());
        let has_line_of_sight = level.sensing_has_line_of_sight(mob, target);
        let had_line_of_sight = self.see_time > 0;
        if has_line_of_sight != had_line_of_sight {
            self.see_time = 0;
        }
        if has_line_of_sight {
            self.see_time += 1;
        } else {
            self.see_time -= 1;
        }
        if !(target_dist_sqr > self.attack_radius_sqr as f64) && self.see_time >= 20 {
            level.mob_mut(mob).expect("a mob").ai.navigation.stop();
            self.strafing_time += 1;
        } else {
            let speed_modifier = self.speed_modifier;
            level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, target, speed_modifier));
            self.strafing_time = -1;
        }
        if self.strafing_time >= 20 {
            let random = &mut level.entity_mut(mob).random;
            if (random.next_float() as f64) < 0.3 {
                self.strafing_clockwise = !self.strafing_clockwise;
            }
            if (random.next_float() as f64) < 0.3 {
                self.strafing_backwards = !self.strafing_backwards;
            }
            self.strafing_time = 0;
        }
        if self.strafing_time > -1 {
            if target_dist_sqr > (self.attack_radius_sqr * 0.75) as f64 {
                self.strafing_backwards = false;
            } else if target_dist_sqr < (self.attack_radius_sqr * 0.25) as f64 {
                self.strafing_backwards = true;
            }
            let forwards = if self.strafing_backwards { -0.5 } else { 0.5 };
            let right = if self.strafing_clockwise { 0.5 } else { -0.5 };
            level.mob_mut(mob).expect("a mob").ai.move_control.strafe(forwards, right);
            if let Some(vehicle) = level.controlled_vehicle(mob).filter(|&vehicle| level.is_mob(vehicle)) {
                level.look_at(vehicle, target, 30.0, 30.0);
            }
            level.look_at(mob, target, 30.0, 30.0);
        } else {
            level.set_look_at_entity(mob, target, 30.0, 30.0);
        }
        if level.is_using_item(mob) {
            if !has_line_of_sight && self.see_time < -60 {
                level.stop_using_item(mob);
            } else if has_line_of_sight {
                let pull_time = level.get_ticks_using_item(mob);
                if pull_time >= 20 {
                    level.stop_using_item(mob);
                    level.perform_ranged_attack(mob, target, bow_item::get_power_for_time(pull_time));
                    self.attack_time = self.attack_interval_min;
                }
            }
        } else {
            self.attack_time -= 1;
            if self.attack_time <= 0 && self.see_time >= -60 {
                let hand = projectile_util::get_weapon_holding_hand(level, mob, items::BOW);
                level.start_using_item(mob, hand);
            }
        }
    }
}
