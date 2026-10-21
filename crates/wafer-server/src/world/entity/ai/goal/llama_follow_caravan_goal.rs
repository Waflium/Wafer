use wafer_core::Vec3;

use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::{EntityId, LevelEntity};

const CARAVAN_LIMIT: i32 = 8;

pub struct LlamaFollowCaravanGoal {
    speed_modifier: f64,
    dist_check_counter: i32,
}

impl LlamaFollowCaravanGoal {
    pub fn new(speed_modifier: f64) -> LlamaFollowCaravanGoal {
        LlamaFollowCaravanGoal { speed_modifier, dist_check_counter: 0 }
    }

    pub fn can_use(&mut self, level: &mut ServerLevel, llama: EntityId) -> bool {
        if level.is_leashed(llama) || level.in_caravan(llama) {
            return false;
        }
        let bb = level.entity(llama).bounding_box().inflate(9.0, 4.0, 9.0);
        let llamas = level.get_entities_matching(Some(llama), &bb, |level, e| level.is_llama(e));
        let mut closest = None;
        let mut closest_dist_square = f64::MAX;
        for &candidate in &llamas {
            if level.in_caravan(candidate) && !level.has_caravan_tail(candidate) {
                let dist_square = level.entity(llama).distance_to_sqr(level.entity(candidate));
                if !(dist_square > closest_dist_square) {
                    closest_dist_square = dist_square;
                    closest = Some(candidate);
                }
            }
        }
        if closest.is_none() {
            for &candidate in &llamas {
                if level.is_leashed(candidate) && !level.has_caravan_tail(candidate) {
                    let dist_square = level.entity(llama).distance_to_sqr(level.entity(candidate));
                    if !(dist_square > closest_dist_square) {
                        closest_dist_square = dist_square;
                        closest = Some(candidate);
                    }
                }
            }
        }
        let Some(closest) = closest else { return false };
        if closest_dist_square < 4.0 {
            return false;
        }
        if !level.is_leashed(closest) && !self.first_is_leashed(level, closest, 1) {
            return false;
        }
        level.join_caravan(llama, closest);
        true
    }

    pub fn can_continue_to_use(&mut self, level: &mut ServerLevel, llama: EntityId) -> bool {
        let Some(head) = level.get_caravan_head(llama) else { return false };
        if level.entity_ref(head).is_some() && level.is_alive(head) && self.first_is_leashed(level, llama, 0) {
            let dist_sqr = level.entity(llama).distance_to_sqr(level.entity(head));
            if dist_sqr > 676.0 {
                if self.speed_modifier <= 3.0 {
                    self.speed_modifier *= 1.2;
                    self.dist_check_counter = reduced_tick_delay(40);
                    return true;
                }
                if self.dist_check_counter == 0 {
                    return false;
                }
            }
            if self.dist_check_counter > 0 {
                self.dist_check_counter -= 1;
            }
            true
        } else {
            false
        }
    }

    pub fn stop(&mut self, level: &mut ServerLevel, llama: EntityId) {
        level.leave_caravan(llama);
        self.speed_modifier = 2.1;
    }

    pub fn tick(&mut self, level: &mut ServerLevel, llama: EntityId) {
        let Some(follows) = level.get_caravan_head(llama).filter(|&head| level.entity_ref(head).is_some()) else { return };
        if level.get_leash_holder(llama).is_some_and(|holder| matches!(level.entities.get(&holder), Some(LevelEntity::LeashKnot(_)))) {
            return;
        }
        let (e, f) = (level.entity(llama), level.entity(follows));
        let distance_to = e.distance_to(f) as f64;
        let delta = Vec3::new(f.x() - e.x(), f.y() - e.y(), f.z() - e.z()).normalize().scale((distance_to - 2.0).max(0.0));
        let (x, y, z) = (e.x() + delta.x, e.y() + delta.y, e.z() + delta.z);
        let speed_modifier = self.speed_modifier;
        level.get_navigation(llama, |navigation, level| navigation.move_to(level, llama, x, y, z, speed_modifier));
    }

    fn first_is_leashed(&self, level: &ServerLevel, current_mob: EntityId, mut counter: i32) -> bool {
        if counter > CARAVAN_LIMIT {
            return false;
        }
        let Some(head) = level.get_caravan_head(current_mob).filter(|&head| level.entity_ref(head).is_some()) else { return false };
        if level.is_leashed(head) {
            true
        } else {
            counter += 1;
            self.first_is_leashed(level, head, counter)
        }
    }
}
