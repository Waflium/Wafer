use wafer_core::Vec3;
use wafer_data::EquipmentSlot;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::InteractionHand;
use crate::world::entity::EntityId;
use crate::world::entity::ai::util::land_random_pos;
use crate::world::item::component::kinetic_weapon::KineticWeapon;
use crate::world::item::component as dc;

const MIN_REPOSITION_DISTANCE: i32 = 6;
const MAX_REPOSITION_DISTANCE: i32 = 7;
const MIN_COOLDOWN_DISTANCE: i32 = 9;
const MAX_COOLDOWN_DISTANCE: i32 = 11;

fn max_fleeing_time() -> f64 {
    reduced_tick_delay(100) as f64
}

pub struct SpearUseGoal {
    state: Option<SpearUseState>,
    speed_modifier_when_charging: f64,
    speed_modifier_when_repositioning: f64,
    approach_distance_sq: f32,
    target_in_range_radius_sq: f32,
}

impl SpearUseGoal {
    pub fn new(speed_modifier_when_charging: f64, speed_modifier_when_repositioning: f64, approach_distance: f32, target_in_range_radius: f32) -> SpearUseGoal {
        SpearUseGoal {
            state: None,
            speed_modifier_when_charging,
            speed_modifier_when_repositioning,
            approach_distance_sq: approach_distance * approach_distance,
            target_in_range_radius_sq: target_in_range_radius * target_in_range_radius,
        }
    }

    pub fn can_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.able_to_attack(level, mob) && !level.is_using_item(mob)
    }

    fn able_to_attack(&self, level: &ServerLevel, mob: EntityId) -> bool {
        level.get_target(mob).is_some() && level.item_by_slot_ref(mob, EquipmentSlot::Mainhand).is_some_and(|item| item.has(dc::KINETIC_WEAPON))
    }

    fn get_kinetic_weapon_use_duration(&self, level: &ServerLevel, mob: EntityId) -> i32 {
        let duration_ticks = level.item_by_slot_ref(mob, EquipmentSlot::Mainhand).and_then(KineticWeapon::of).map_or(0, |kinetic_weapon| kinetic_weapon.compute_damage_use_duration());
        reduced_tick_delay(duration_ticks)
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.state.as_ref().is_some_and(|state| !state.done) && self.able_to_attack(level, mob)
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        level.mob_mut(mob).expect("a mob").set_aggressive(true);
        self.state = Some(SpearUseState::default());
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let m = level.mob_mut(mob).expect("a mob");
        m.ai.navigation.stop();
        m.set_aggressive(false);
        self.state = None;
        level.stop_using_item(mob);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        if self.state.is_none() {
            return;
        }
        let Some(target) = level.get_target(mob) else { return };
        let t = level.entity(target);
        let (target_x, target_y, target_z, target_position) = (t.x(), t.y(), t.z(), t.position());
        let target_dist_sqr = level.entity(mob).distance_to_sqr_xyz(target_x, target_y, target_z);
        let mount = level.root_vehicle(mob);
        let speed_modifier = if level.mob(mount).is_some() { level.charge_speed_modifier(mount) } else { 1.0 };
        let mount_distance = if level.is_passenger(mob) { 2 } else { 0 };
        level.look_at(mob, target, 30.0, 30.0);
        level.set_look_at_entity(mob, target, 30.0, 30.0);
        let state = self.state.as_mut().expect("a spear use state");
        if state.not_engaged_yet() {
            if target_dist_sqr > self.approach_distance_sq as f64 {
                let speed = speed_modifier as f64 * self.speed_modifier_when_repositioning;
                level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, target, speed));
                return;
            }
            let duration = self.get_kinetic_weapon_use_duration(level, mob);
            self.state.as_mut().expect("a spear use state").start_engagement(duration);
            level.start_using_item(mob, InteractionHand::MainHand);
        }
        if self.state.as_mut().expect("a spear use state").tick_and_check_engagement() {
            level.stop_using_item(mob);
            let distance = target_dist_sqr.sqrt();
            let away_pos = land_random_pos::get_pos_away_between(
                level,
                mob,
                0.0f64.max((MIN_COOLDOWN_DISTANCE + mount_distance) as f64 - distance),
                1.0f64.max((MAX_COOLDOWN_DISTANCE + mount_distance) as f64 - distance),
                7,
                target_position,
            );
            let state = self.state.as_mut().expect("a spear use state");
            state.away_pos = away_pos;
            state.fleeing_time = 1;
        }
        if self.state.as_mut().expect("a spear use state").tick_and_check_fleeing() {
            return;
        }
        match self.state.as_ref().expect("a spear use state").away_pos {
            Some(away_pos) => {
                let speed = speed_modifier as f64 * self.speed_modifier_when_repositioning;
                level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, away_pos.x, away_pos.y, away_pos.z, speed));
                if level.mob(mob).expect("a mob").ai.navigation.is_done() {
                    let state = self.state.as_mut().expect("a spear use state");
                    if state.fleeing_time > 0 {
                        state.done = true;
                        return;
                    }
                    state.away_pos = None;
                }
            }
            None => {
                let speed = speed_modifier as f64 * self.speed_modifier_when_charging;
                level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, target, speed));
                if target_dist_sqr < self.target_in_range_radius_sq as f64 || level.mob(mob).expect("a mob").ai.navigation.is_done() {
                    let distance = target_dist_sqr.sqrt();
                    let away_pos = land_random_pos::get_pos_away_between(
                        level,
                        mob,
                        (MIN_REPOSITION_DISTANCE + mount_distance) as f64 - distance,
                        (MAX_REPOSITION_DISTANCE + mount_distance) as f64 - distance,
                        7,
                        target_position,
                    );
                    self.state.as_mut().expect("a spear use state").away_pos = away_pos;
                }
            }
        }
    }
}

struct SpearUseState {
    engage_time: i32,
    fleeing_time: i32,
    away_pos: Option<Vec3>,
    done: bool,
}

impl Default for SpearUseState {
    fn default() -> SpearUseState {
        SpearUseState { engage_time: -1, fleeing_time: -1, away_pos: None, done: false }
    }
}

impl SpearUseState {
    fn not_engaged_yet(&self) -> bool {
        self.engage_time < 0
    }

    fn start_engagement(&mut self, spear_down_time: i32) {
        self.engage_time = spear_down_time;
    }

    fn tick_and_check_engagement(&mut self) -> bool {
        if self.engage_time > 0 {
            self.engage_time -= 1;
            if self.engage_time == 0 {
                return true;
            }
        }
        false
    }

    fn tick_and_check_fleeing(&mut self) -> bool {
        if self.fleeing_time > 0 {
            self.fleeing_time += 1;
            if self.fleeing_time as f64 > max_fleeing_time() {
                self.done = true;
                return true;
            }
        }
        false
    }
}
