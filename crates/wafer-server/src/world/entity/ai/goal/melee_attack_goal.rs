use wafer_core::random::RandomSource;

use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::entity::entity_selector;
use crate::world::level::pathfinder::path::Path;

const COOLDOWN_BETWEEN_CAN_USE_CHECKS: i64 = 20;

pub struct MeleeAttackGoal {
    speed_modifier: f64,
    following_target_even_if_not_seen: bool,
    path: Option<Path>,
    pathed_target_x: f64,
    pathed_target_y: f64,
    pathed_target_z: f64,
    ticks_until_next_path_recalculation: i32,
    ticks_until_next_attack: i32,
    attack_interval: i32,
    last_can_use_check: i64,
}

impl MeleeAttackGoal {
    pub fn new(speed_modifier: f64, following_target_even_if_not_seen: bool) -> MeleeAttackGoal {
        MeleeAttackGoal {
            speed_modifier,
            following_target_even_if_not_seen,
            path: None,
            pathed_target_x: 0.0,
            pathed_target_y: 0.0,
            pathed_target_z: 0.0,
            ticks_until_next_path_recalculation: 0,
            ticks_until_next_attack: 0,
            attack_interval: 20,
            last_can_use_check: 0,
        }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let time = level.game_time;
        if time - self.last_can_use_check < COOLDOWN_BETWEEN_CAN_USE_CHECKS {
            return false;
        }
        self.last_can_use_check = time;
        let Some(target) = level.get_target(mob) else { return false };
        if !level.is_alive(target) {
            return false;
        }
        self.path = level.get_navigation(mob, |navigation, level| navigation.create_path_to_entity(level, mob, target, 0));
        if self.path.is_some() { true } else { level.is_within_melee_attack_range(mob, target) }
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, mob: EntityId) -> bool {
        let Some(target) = level.get_target(mob) else { return false };
        if !level.is_alive(target) {
            false
        } else if !self.following_target_even_if_not_seen {
            !level.mob(mob).expect("a mob").ai.navigation.is_done()
        } else if !level.mob(mob).expect("a mob").is_within_home(level.entity(target).block_position()) {
            false
        } else {
            !(level.is_player(target) && (level.is_spectator(target) || level.player(target).game_mode.game_type.is_creative()))
        }
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let (path, speed_modifier) = (self.path.take(), self.speed_modifier);
        level.get_navigation(mob, |navigation, level| navigation.move_to_path(level, mob, path, speed_modifier));
        level.mob_mut(mob).expect("a mob").set_aggressive(true);
        self.ticks_until_next_path_recalculation = 0;
        self.ticks_until_next_attack = 0;
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let target = level.get_target(mob);
        if !target.is_none_or(|target| entity_selector::no_creative_or_spectator(level, target)) {
            level.set_target(mob, None);
        }
        let m = level.mob_mut(mob).expect("a mob");
        m.set_aggressive(false);
        m.ai.navigation.stop();
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(target) = level.get_target(mob) else { return };
        level.set_look_at_entity(mob, target, 30.0, 30.0);
        self.ticks_until_next_path_recalculation = (self.ticks_until_next_path_recalculation - 1).max(0);
        if (self.following_target_even_if_not_seen || level.sensing_has_line_of_sight(mob, target)) && self.ticks_until_next_path_recalculation <= 0 && {
            let (x, y, z) = (self.pathed_target_x, self.pathed_target_y, self.pathed_target_z);
            x == 0.0 && y == 0.0 && z == 0.0 || level.entity(target).distance_to_sqr_xyz(x, y, z) >= 1.0 || level.entity_mut(mob).random.next_float() < 0.05
        } {
            let t = level.entity(target);
            self.pathed_target_x = t.x();
            self.pathed_target_y = t.y();
            self.pathed_target_z = t.z();
            self.ticks_until_next_path_recalculation = 4 + level.entity_mut(mob).random.next_int_bounded(7);
            let target_distance_sqr = level.entity(mob).distance_to_sqr(level.entity(target));
            if target_distance_sqr > 1024.0 {
                self.ticks_until_next_path_recalculation += 10;
            } else if target_distance_sqr > 256.0 {
                self.ticks_until_next_path_recalculation += 5;
            }
            let speed_modifier = self.speed_modifier;
            if !level.get_navigation(mob, |navigation, level| navigation.move_to_entity_with_reach_range(level, mob, target, 0, speed_modifier)) {
                self.ticks_until_next_path_recalculation += 15;
            }
            self.ticks_until_next_path_recalculation = self.adjusted_tick_delay(self.ticks_until_next_path_recalculation);
        }
        self.ticks_until_next_attack = (self.ticks_until_next_attack - 1).max(0);
        self.check_and_perform_attack(level, mob, target);
    }

    fn adjusted_tick_delay(&self, ticks: i32) -> i32 {
        ticks
    }

    fn check_and_perform_attack(&mut self, level: &mut ServerLevel, mob: EntityId, target: EntityId) {
        if self.can_perform_attack(level, mob, target) {
            self.reset_attack_cooldown();
            level.swing_for_attack(mob, InteractionHand::MainHand);
            level.do_hurt_target(mob, target);
        }
    }

    fn reset_attack_cooldown(&mut self) {
        self.ticks_until_next_attack = self.adjusted_tick_delay(20);
    }

    fn is_time_to_attack(&self) -> bool {
        self.ticks_until_next_attack <= 0
    }

    fn can_perform_attack(&mut self, level: &mut ServerLevel, mob: EntityId, target: EntityId) -> bool {
        self.is_time_to_attack() && level.is_within_melee_attack_range(mob, target) && level.sensing_has_line_of_sight(mob, target)
    }

    pub fn get_ticks_until_next_attack(&self) -> i32 {
        self.ticks_until_next_attack
    }

    pub fn get_attack_interval(&self) -> i32 {
        self.adjusted_tick_delay(self.attack_interval)
    }
}
