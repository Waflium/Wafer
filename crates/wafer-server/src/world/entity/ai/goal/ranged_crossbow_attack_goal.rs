use wafer_core::random::RandomSource;
use wafer_data::items;

use crate::server::level::ServerLevel;
use crate::util::time_util;
use crate::util::valueproviders::IntProvider;
use crate::world::entity::EntityId;
use crate::world::entity::projectile::projectile_util;
use crate::world::item::crossbow_item;

pub const PATHFINDING_DELAY_RANGE: IntProvider = time_util::range_of_seconds(1, 2);

#[derive(Clone, Copy, PartialEq, Eq)]
enum CrossbowState {
    Uncharged,
    Charging,
    Charged,
    ReadyToAttack,
}

pub struct RangedCrossbowAttackGoal {
    crossbow_state: CrossbowState,
    speed_modifier: f64,
    attack_radius_sqr: f32,
    see_time: i32,
    attack_delay: i32,
    update_path_delay: i32,
}

impl RangedCrossbowAttackGoal {
    pub fn new(speed_modifier: f64, attack_radius: f32) -> RangedCrossbowAttackGoal {
        RangedCrossbowAttackGoal {
            crossbow_state: CrossbowState::Uncharged,
            speed_modifier,
            attack_radius_sqr: attack_radius * attack_radius,
            see_time: 0,
            attack_delay: 0,
            update_path_delay: 0,
        }
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.is_valid_target(level, mob) && self.is_holding_crossbow(level, mob)
    }

    fn is_holding_crossbow(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.is_holding(mob, items::CROSSBOW)
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.is_valid_target(level, mob) && (self.can_use(level, mob) || !level.mob(mob).expect("a mob").ai.navigation.is_done()) && self.is_holding_crossbow(level, mob)
    }

    fn is_valid_target(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_some_and(|target| level.is_alive(target))
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").set_aggressive(false);
        level.set_target(mob, None);
        self.see_time = 0;
        if level.is_using_item(mob) {
            level.stop_using_item(mob);
            level.set_charging_crossbow(mob, false);
        }
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
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
        let t = level.entity(target);
        let distance_to_sqr = level.entity(mob).distance_to_sqr_xyz(t.x(), t.y(), t.z());
        let needs_to_move = (distance_to_sqr > self.attack_radius_sqr as f64 || self.see_time < 5) && self.attack_delay == 0;
        if needs_to_move {
            self.update_path_delay -= 1;
            if self.update_path_delay <= 0 {
                let speed_modifier = if self.can_run() { self.speed_modifier } else { self.speed_modifier * 0.5 };
                level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, target, speed_modifier));
                self.update_path_delay = PATHFINDING_DELAY_RANGE.sample(&mut level.entity_mut(mob).random);
            }
        } else {
            self.update_path_delay = 0;
            level.mob_mut(mob).expect("a mob").ai.navigation.stop();
        }
        level.set_look_at_entity(mob, target, 30.0, 30.0);
        match self.crossbow_state {
            CrossbowState::Uncharged => {
                if !needs_to_move {
                    let hand = projectile_util::get_weapon_holding_hand(level, mob, items::CROSSBOW);
                    level.start_using_item(mob, hand);
                    self.crossbow_state = CrossbowState::Charging;
                    level.set_charging_crossbow(mob, true);
                }
            }
            CrossbowState::Charging => {
                if !level.is_using_item(mob) {
                    self.crossbow_state = CrossbowState::Uncharged;
                }
                let pull_time = level.get_ticks_using_item(mob);
                let use_item = level.get_use_item(mob);
                if pull_time >= crossbow_item::get_charge_duration(level, &use_item, mob) {
                    level.release_using_item(mob);
                    self.crossbow_state = CrossbowState::Charged;
                    self.attack_delay = 20 + level.entity_mut(mob).random.next_int_bounded(20);
                    level.set_charging_crossbow(mob, false);
                }
            }
            CrossbowState::Charged => {
                self.attack_delay -= 1;
                if self.attack_delay == 0 {
                    self.crossbow_state = CrossbowState::ReadyToAttack;
                }
            }
            CrossbowState::ReadyToAttack => {
                if has_line_of_sight {
                    level.perform_ranged_attack(mob, target, 1.0);
                    self.crossbow_state = CrossbowState::Uncharged;
                }
            }
        }
    }

    fn can_run(&self) -> bool {
        self.crossbow_state == CrossbowState::Uncharged
    }
}
