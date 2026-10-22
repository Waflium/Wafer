use super::goal::reduced_tick_delay;
use crate::server::level::ServerLevel;
use crate::world::entity::EntityId;
use crate::world::level::pathfinder::path_type::PathType;

pub struct FollowMobGoal {
    following_mob: Option<EntityId>,
    speed_modifier: f64,
    time_to_recalc_path: i32,
    stop_distance: f32,
    old_water_cost: f32,
    area_size: f32,
}

impl FollowMobGoal {
    pub fn new(speed_modifier: f64, stop_distance: f32, area_size: f32) -> FollowMobGoal {
        FollowMobGoal { following_mob: None, speed_modifier, time_to_recalc_path: 0, stop_distance, old_water_cost: 0.0, area_size }
    }

    pub fn can_use(&mut self, level: &ServerLevel, mob: EntityId) -> bool {
        let kind = std::mem::discriminant(&level.mob(mob).expect("a mob").kind);
        let area = level.entity(mob).bounding_box().inflate_all(self.area_size as f64);
        let mobs = level.get_entities(None, &area).into_iter().filter(|&input| level.mob(input).is_some_and(|other| std::mem::discriminant(&other.kind) != kind));
        for mob_in_list in mobs {
            if !level.entity(mob_in_list).is_invisible() {
                self.following_mob = Some(mob_in_list);
                return true;
            }
        }
        false
    }

    pub fn can_continue_to_use(&self, level: &ServerLevel, mob: EntityId) -> bool {
        self.following_mob.is_some_and(|following_mob| {
            !level.mob(mob).expect("a mob").ai.navigation.is_done() && level.entity(mob).distance_to_sqr(level.entity(following_mob)) > (self.stop_distance * self.stop_distance) as f64
        })
    }

    pub fn start(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.time_to_recalc_path = 0;
        let m = level.mob(mob).expect("a mob");
        self.old_water_cost = m.get_pathfinding_malus(PathType::Water);
        m.set_pathfinding_malus(PathType::Water, 0.0);
    }

    pub fn stop(&mut self, level: &mut ServerLevel, mob: EntityId) {
        self.following_mob = None;
        level.get_navigation(mob, |navigation, _| navigation.stop());
        level.mob(mob).expect("a mob").set_pathfinding_malus(PathType::Water, self.old_water_cost);
    }

    pub fn tick(&mut self, level: &mut ServerLevel, mob: EntityId) {
        let Some(following_mob) = self.following_mob else { return };
        if level.is_leashed(mob) {
            return;
        }
        let max_head_x_rot = level.mob(mob).expect("a mob").get_max_head_x_rot() as f32;
        level.set_look_at_entity(mob, following_mob, 10.0, max_head_x_rot);
        self.time_to_recalc_path -= 1;
        if self.time_to_recalc_path > 0 {
            return;
        }
        self.time_to_recalc_path = reduced_tick_delay(10);
        let (m, f) = (level.entity(mob), level.entity(following_mob));
        let (xxd, yyd, zzd) = (m.x() - f.x(), m.y() - f.y(), m.z() - f.z());
        let dist_sqr = xxd * xxd + yyd * yyd + zzd * zzd;
        let speed_modifier = self.speed_modifier;
        if !(dist_sqr <= (self.stop_distance * self.stop_distance) as f64) {
            level.get_navigation(mob, |navigation, level| navigation.move_to_entity(level, mob, following_mob, speed_modifier));
        } else {
            level.get_navigation(mob, |navigation, _| navigation.stop());
            let look_control = &level.mob(following_mob).expect("a followed mob").ai.look_control;
            let m = level.entity(mob);
            if dist_sqr <= self.stop_distance as f64 || look_control.wanted_x == m.x() && look_control.wanted_y == m.y() && look_control.wanted_z == m.z() {
                let f = level.entity(following_mob);
                let (delta_x, delta_z) = (f.x() - m.x(), f.z() - m.z());
                let (x, y, z) = (m.x() - delta_x, m.y(), m.z() - delta_z);
                level.get_navigation(mob, |navigation, level| navigation.move_to(level, mob, x, y, z, speed_modifier));
            }
        }
    }
}
